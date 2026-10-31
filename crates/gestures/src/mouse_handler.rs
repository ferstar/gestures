use crate::xdo_dyn::DynamicXDo;
use chrono::Duration;
use evdev::{
    uinput::VirtualDevice, AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode,
    SynchronizationCode,
};
use std::env;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::mpsc::TrySendError;
use std::thread;
use std::time::Duration as StdDuration;
use std::time::Instant;
use timer::Timer;

fn current_uid() -> Option<u32> {
    std::fs::metadata("/proc/self").ok().map(|m| m.uid())
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MouseCommand {
    MouseUp,
    MouseDown,
    MoveMouseRelative,
    Shutdown,
}

pub struct MouseHandler {
    tx: Option<mpsc::SyncSender<(MouseCommand, i32, i32)>>,
    worker: Option<thread::JoinHandle<()>>,
    timer: Timer,
    guard: Option<timer::Guard>,
    dropped_move_events: u64,
    last_drop_report: Instant,
}

/// Try to setup X11 environment variables by detecting XAUTHORITY file
fn setup_x11_env() {
    // Ensure DISPLAY is set
    if env::var("DISPLAY").is_err() {
        env::set_var("DISPLAY", ":0");
        log::info!("DISPLAY not set, using default :0");
    }

    // Check if XAUTHORITY is already set and valid
    if let Ok(xauth) = env::var("XAUTHORITY") {
        if Path::new(&xauth).exists() {
            log::debug!("XAUTHORITY already set to: {}", xauth);
            return;
        }
    }

    // Try to find XAUTHORITY in common locations
    let home = env::var("HOME").unwrap_or_default();
    let possible_paths = vec![
        format!("{}/.Xauthority", home),
        "/tmp/.Xauthority".to_string(),
    ];

    // Also check /tmp for dynamic xauth files (pattern: /tmp/xauth_*)
    if let Ok(entries) = std::fs::read_dir("/tmp") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().starts_with("xauth_") {
                    let Ok(metadata) = entry.metadata() else {
                        continue;
                    };
                    let Some(uid) = current_uid() else {
                        continue;
                    };
                    if metadata.uid() != uid || !path.is_file() {
                        continue;
                    }
                    if let Some(path_str) = path.to_str() {
                        env::set_var("XAUTHORITY", path_str);
                        log::info!("Set XAUTHORITY to: {}", path_str);
                        return;
                    }
                }
            }
        }
    }

    // Try the standard locations
    for path in possible_paths {
        if Path::new(&path).exists() {
            env::set_var("XAUTHORITY", &path);
            log::info!("Set XAUTHORITY to: {}", path);
            return;
        }
    }

    log::warn!("Could not find XAUTHORITY file, X11 initialization may fail");
}

const MOUSE_EVENT_QUEUE_SIZE: usize = 64;
type MouseEvent = (MouseCommand, i32, i32);

enum MouseBackend {
    X11(DynamicXDo),
    Uinput(VirtualDevice),
    Ydotool,
}

impl MouseBackend {
    fn new(is_xorg: bool) -> Self {
        if is_xorg {
            match DynamicXDo::new() {
                Ok(xdo) => {
                    log::info!("Successfully initialized libxdo for X11");
                    return Self::X11(xdo);
                }
                Err(e) => {
                    log::warn!("Failed to initialize libxdo: {e}; trying uinput");
                }
            }
        }
        match create_virtual_pointer() {
            Ok(device) => {
                log::info!("Using uinput virtual pointer");
                Self::Uinput(device)
            }
            Err(e) => {
                log::warn!(
                    "Cannot create /dev/uinput virtual pointer: {e}; falling back to ydotool. \
                     Ensure the uinput module is loaded and grant /dev/uinput access via the \
                     input/uinput group or a udev rule such as \
                     KERNEL==\"uinput\", GROUP=\"input\", MODE=\"0660\"; \
                     re-login after changing groups. Fallback requires ydotool and ydotoold."
                );
                Self::Ydotool
            }
        }
    }

    fn execute(&mut self, command: MouseCommand, param1: i32, param2: i32) {
        match self {
            Self::X11(xdo) => {
                let result = match command {
                    MouseCommand::MouseDown => xdo.mouse_down(param1),
                    MouseCommand::MouseUp => xdo.mouse_up(param1),
                    MouseCommand::MoveMouseRelative => xdo.move_mouse_relative(param1, param2),
                    MouseCommand::Shutdown => unreachable!(),
                };
                if let Err(e) = result {
                    log::warn!("libxdo mouse command failed: {e:?}");
                }
            }
            Self::Uinput(device) => {
                let result = mouse_event_frame(command, param1, param2).and_then(|frame| {
                    // emit appends SYN_REPORT itself; omit the frame's final SYN to avoid two.
                    device.emit(&frame[..frame.len() - 1])
                });
                if let Err(e) = result {
                    log::warn!("uinput mouse command failed: {e}");
                }
            }
            Self::Ydotool => {
                if let Err(e) = execute_ydotool("ydotool", command, param1, param2) {
                    log::warn!("ydotool mouse command failed: {e}");
                }
            }
        }
    }
}

fn create_virtual_pointer() -> std::io::Result<VirtualDevice> {
    VirtualDevice::builder()?
        .name("gestures-virtual-pointer")
        .with_keys(&AttributeSet::from_iter([
            KeyCode::BTN_LEFT,
            KeyCode::BTN_RIGHT,
            KeyCode::BTN_MIDDLE,
        ]))?
        .with_relative_axes(&AttributeSet::from_iter([
            RelativeAxisCode::REL_X,
            RelativeAxisCode::REL_Y,
        ]))?
        .build()
}

fn mouse_button(button: i32) -> std::io::Result<KeyCode> {
    // MouseHandler uses X11 numbering, not ydotool's zero-based button indices.
    match button {
        1 => Ok(KeyCode::BTN_LEFT),
        2 => Ok(KeyCode::BTN_MIDDLE),
        3 => Ok(KeyCode::BTN_RIGHT),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Unsupported mouse button: {button}"),
        )),
    }
}

/// Build a complete evdev frame. For button commands param2 is a legacy placeholder (255).
fn mouse_event_frame(
    command: MouseCommand,
    param1: i32,
    param2: i32,
) -> std::io::Result<Vec<InputEvent>> {
    let mut events = Vec::with_capacity(3);
    match command {
        MouseCommand::MoveMouseRelative => {
            if param1 != 0 {
                events.push(InputEvent::new(
                    EventType::RELATIVE.0,
                    RelativeAxisCode::REL_X.0,
                    param1,
                ));
            }
            if param2 != 0 {
                events.push(InputEvent::new(
                    EventType::RELATIVE.0,
                    RelativeAxisCode::REL_Y.0,
                    param2,
                ));
            }
        }
        MouseCommand::MouseDown | MouseCommand::MouseUp => {
            events.push(InputEvent::new(
                EventType::KEY.0,
                mouse_button(param1)?.0,
                i32::from(command == MouseCommand::MouseDown),
            ));
        }
        MouseCommand::Shutdown => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Shutdown is a worker control command, not an input event",
            ));
        }
    }
    events.push(InputEvent::new(
        EventType::SYNCHRONIZATION.0,
        SynchronizationCode::SYN_REPORT.0,
        0,
    ));
    Ok(events)
}

pub fn start_handler(is_xorg: bool) -> MouseHandler {
    if is_xorg {
        setup_x11_env();
    }
    let (tx, rx) = mpsc::sync_channel(MOUSE_EVENT_QUEUE_SIZE);
    let worker = thread::spawn(move || {
        let mut backend = MouseBackend::new(is_xorg);
        mouse_worker(rx, |command, param1, param2| {
            backend.execute(command, param1, param2);
        });
    });

    MouseHandler {
        tx: Some(tx),
        worker: Some(worker),
        timer: Timer::new(),
        guard: None,
        dropped_move_events: 0,
        last_drop_report: Instant::now(),
    }
}

fn execute_ydotool(
    program: impl AsRef<std::ffi::OsStr>,
    command: MouseCommand,
    param1: i32,
    param2: i32,
) -> std::io::Result<()> {
    let mut process = Command::new(program);
    match command {
        MouseCommand::MouseDown | MouseCommand::MouseUp => {
            let index = match mouse_button(param1)? {
                KeyCode::BTN_LEFT => 0,
                KeyCode::BTN_RIGHT => 1,
                KeyCode::BTN_MIDDLE => 2,
                _ => unreachable!(),
            };
            // ydotool: 0x40 = press, 0x80 = release; low bits select the button.
            let action = if command == MouseCommand::MouseDown {
                0x40
            } else {
                0x80
            };
            process.args(["click", "--", &format!("0x{:x}", action | index)]);
        }
        MouseCommand::MoveMouseRelative => {
            process.args([
                "mousemove",
                "-x",
                &param1.to_string(),
                "-y",
                &param2.to_string(),
            ]);
        }
        MouseCommand::Shutdown => unreachable!(),
    }
    // Wait on the worker, never the input loop, and reap every child we launch.
    let status = process.stdout(Stdio::null()).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("exited with {status}")))
    }
}

fn mouse_worker(rx: mpsc::Receiver<MouseEvent>, mut execute: impl FnMut(MouseCommand, i32, i32)) {
    let mut pending = None;
    let mut pressed_button = None;
    while let Some((command, mut x, mut y)) = pending.take().or_else(|| rx.recv().ok()) {
        if command == MouseCommand::Shutdown {
            break;
        }
        if command == MouseCommand::MoveMouseRelative {
            while let Ok((next, next_x, next_y)) = rx.try_recv() {
                if next == MouseCommand::MoveMouseRelative {
                    x = x.saturating_add(next_x);
                    y = y.saturating_add(next_y);
                } else {
                    pending = Some((next, next_x, next_y));
                    break;
                }
            }
        }
        execute(command, x, y);
        match command {
            MouseCommand::MouseDown => pressed_button = Some(x),
            MouseCommand::MouseUp => pressed_button = None,
            _ => {}
        }
    }
    if let Some(button) = pressed_button {
        execute(MouseCommand::MouseUp, button, 255);
    }
}

impl Drop for MouseHandler {
    fn drop(&mut self) {
        self.cancel_timer_if_present();
        if let Some(tx) = self.tx.take() {
            let _ = tx.send((MouseCommand::Shutdown, 0, 0));
        }
        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() {
                log::error!("Mouse worker panicked during shutdown");
            }
        }
    }
}

impl MouseHandler {
    pub fn mouse_down(&mut self, button: i32) {
        self.cancel_timer_if_present();
        if let Some(ref tx) = self.tx {
            let _ = tx.send((MouseCommand::MouseDown, button, 255));
        }
    }

    pub fn mouse_up_delay(&mut self, button: i32, delay_ms: i64) {
        if let Some(ref tx) = self.tx {
            let tx_clone = tx.clone();
            self.guard = Some(self.timer.schedule_with_delay(
                Duration::milliseconds(delay_ms),
                move || {
                    let _ = tx_clone.send((MouseCommand::MouseUp, button, 255));
                },
            ));
        }
    }

    pub fn move_mouse_relative(&mut self, x_val: i32, y_val: i32) -> bool {
        if x_val == 0 && y_val == 0 {
            return true;
        }

        self.cancel_timer_if_present();
        if let Some(ref tx) = self.tx {
            match tx.try_send((MouseCommand::MoveMouseRelative, x_val, y_val)) {
                Ok(()) => return true,
                Err(TrySendError::Full(_)) => {
                    self.dropped_move_events = self.dropped_move_events.saturating_add(1);
                    self.maybe_report_drop_stats();
                }
                Err(TrySendError::Disconnected(_)) => {
                    log::warn!("Mouse worker disconnected, dropping move event");
                }
            }
        }
        false
    }

    fn cancel_timer_if_present(&mut self) {
        if self.guard.is_some() {
            self.guard = None;
        }
    }

    fn maybe_report_drop_stats(&mut self) {
        if !log::log_enabled!(log::Level::Debug) || self.dropped_move_events == 0 {
            return;
        }
        if self.last_drop_report.elapsed() >= StdDuration::from_secs(10) {
            log::debug!(
                "mouse move queue stats: dropped_move_events={}",
                self.dropped_move_events
            );
            self.dropped_move_events = 0;
            self.last_drop_report = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{Arc, Mutex};

    fn event_values(events: &[InputEvent]) -> Vec<(EventType, u16, i32)> {
        events
            .iter()
            .map(|event| (event.event_type(), event.code(), event.value()))
            .collect()
    }

    #[test]
    fn uinput_move_frame_omits_zero_axes_and_ends_with_one_sync() {
        let syn = (
            EventType::SYNCHRONIZATION,
            SynchronizationCode::SYN_REPORT.0,
            0,
        );
        for (x, y, expected) in [
            (
                7,
                -4,
                vec![
                    (EventType::RELATIVE, RelativeAxisCode::REL_X.0, 7),
                    (EventType::RELATIVE, RelativeAxisCode::REL_Y.0, -4),
                    syn,
                ],
            ),
            (
                0,
                -4,
                vec![(EventType::RELATIVE, RelativeAxisCode::REL_Y.0, -4), syn],
            ),
            (
                7,
                0,
                vec![(EventType::RELATIVE, RelativeAxisCode::REL_X.0, 7), syn],
            ),
            (0, 0, vec![syn]),
        ] {
            let frame = mouse_event_frame(MouseCommand::MoveMouseRelative, x, y).unwrap();
            assert_eq!(event_values(&frame), expected);
        }
    }

    #[test]
    fn uinput_button_frames_map_x11_numbers_and_ignore_legacy_placeholder() {
        for (button, key) in [
            (1, KeyCode::BTN_LEFT),
            (2, KeyCode::BTN_MIDDLE),
            (3, KeyCode::BTN_RIGHT),
        ] {
            for (command, value) in [(MouseCommand::MouseDown, 1), (MouseCommand::MouseUp, 0)] {
                for placeholder in [0, 255] {
                    let frame = mouse_event_frame(command, button, placeholder).unwrap();
                    assert_eq!(
                        event_values(&frame),
                        [
                            (EventType::KEY, key.0, value),
                            (
                                EventType::SYNCHRONIZATION,
                                SynchronizationCode::SYN_REPORT.0,
                                0
                            ),
                        ]
                    );
                }
            }
        }
        assert!(mouse_event_frame(MouseCommand::MouseDown, 4, 255).is_err());
        assert!(mouse_event_frame(MouseCommand::Shutdown, 0, 0).is_err());
    }

    #[test]
    fn coalesced_worker_moves_produce_one_uinput_frame_before_release() {
        let (tx, rx) = mpsc::sync_channel(8);
        for event in [
            (MouseCommand::MouseDown, 1, 255),
            (MouseCommand::MoveMouseRelative, 2, 3),
            (MouseCommand::MoveMouseRelative, -1, -3),
            (MouseCommand::Shutdown, 0, 0),
        ] {
            tx.send(event).unwrap();
        }
        let mut frames = Vec::new();
        mouse_worker(rx, |command, x, y| {
            frames.push(event_values(&mouse_event_frame(command, x, y).unwrap()));
        });
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0][0], (EventType::KEY, KeyCode::BTN_LEFT.0, 1));
        assert_eq!(
            frames[1],
            [
                (EventType::RELATIVE, RelativeAxisCode::REL_X.0, 1),
                (
                    EventType::SYNCHRONIZATION,
                    SynchronizationCode::SYN_REPORT.0,
                    0,
                ),
            ]
        );
        assert_eq!(frames[2][0], (EventType::KEY, KeyCode::BTN_LEFT.0, 0));
    }

    #[test]
    fn worker_coalesces_moves_without_crossing_button_events() {
        let (tx, rx) = mpsc::sync_channel(16);
        let events = [
            (MouseCommand::MouseDown, 1, 255),
            (MouseCommand::MoveMouseRelative, 2, 3),
            (MouseCommand::MoveMouseRelative, -1, 4),
            (MouseCommand::MouseUp, 1, 255),
            (MouseCommand::MoveMouseRelative, 8, 9),
            (MouseCommand::Shutdown, 0, 0),
        ];
        for event in events {
            tx.send(event).unwrap();
        }
        let mut executed = Vec::new();
        mouse_worker(rx, |command, x, y| executed.push((command, x, y)));
        assert_eq!(
            executed,
            [
                events[0],
                (MouseCommand::MoveMouseRelative, 1, 7),
                events[3],
                events[4],
            ]
        );
    }

    #[test]
    fn disconnected_worker_releases_held_button() {
        let (tx, rx) = mpsc::sync_channel(2);
        tx.send((MouseCommand::MouseDown, 1, 255)).unwrap();
        drop(tx);
        let mut executed = Vec::new();
        mouse_worker(rx, |command, x, y| executed.push((command, x, y)));
        assert_eq!(
            executed,
            [
                (MouseCommand::MouseDown, 1, 255),
                (MouseCommand::MouseUp, 1, 255),
            ]
        );
    }

    #[test]
    fn dropping_handler_cancels_timer_drains_queue_and_releases_button() {
        let (tx, rx) = mpsc::sync_channel(4);
        let executed = Arc::new(Mutex::new(Vec::new()));
        let output = executed.clone();
        let worker = thread::spawn(move || {
            mouse_worker(rx, |command, x, y| {
                output.lock().unwrap().push((command, x, y));
            });
        });
        let mut handler = MouseHandler {
            tx: Some(tx),
            worker: Some(worker),
            timer: Timer::new(),
            guard: None,
            dropped_move_events: 0,
            last_drop_report: Instant::now(),
        };
        handler.mouse_down(1);
        handler.move_mouse_relative(5, -3);
        handler.mouse_up_delay(1, 60_000);
        drop(handler);
        assert_eq!(
            *executed.lock().unwrap(),
            [
                (MouseCommand::MouseDown, 1, 255),
                (MouseCommand::MoveMouseRelative, 5, -3),
                (MouseCommand::MouseUp, 1, 255),
            ]
        );
    }

    #[test]
    fn ydotool_children_are_reaped_even_when_commands_fail() {
        let dir = env::temp_dir().join(format!(
            "gestures-reaping-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let program = dir.join("ydotool");
        let pids = dir.join("pids");
        for exit_code in [0, 1] {
            std::fs::write(
                &program,
                format!(
                    "#!/bin/sh\nprintf '%s\\n' \"$$\" >> '{}'\nexit {exit_code}\n",
                    pids.display()
                ),
            )
            .unwrap();
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
            for _ in 0..100 {
                let result = execute_ydotool(&program, MouseCommand::MoveMouseRelative, 1, -1);
                assert_eq!(result.is_ok(), exit_code == 0);
            }
        }
        let pids = std::fs::read_to_string(pids).unwrap();
        assert_eq!(pids.lines().count(), 200);
        for pid in pids.lines() {
            assert!(
                !Path::new(&format!("/proc/{pid}")).exists(),
                "child {pid} was not reaped"
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
