use chrono::Duration;
use libxdo::XDo;
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

pub fn start_handler(is_xorg: bool) -> MouseHandler {
    if is_xorg {
        setup_x11_env();
    }
    let (tx, rx) = mpsc::sync_channel(MOUSE_EVENT_QUEUE_SIZE);
    let worker = thread::spawn(move || {
        let xdo = if is_xorg {
            match XDo::new(None) {
                Ok(xdo) => {
                    log::info!("Successfully initialized libxdo for X11");
                    Some(xdo)
                }
                Err(e) => {
                    log::error!("Failed to initialize libxdo: {:?}", e);
                    log::warn!("Falling back to ydotool mouse control mode");
                    None
                }
            }
        } else {
            None
        };
        mouse_worker(rx, |command, param1, param2| {
            if let Some(ref xdo) = xdo {
                let result = match command {
                    MouseCommand::MouseDown => xdo.mouse_down(param1),
                    MouseCommand::MouseUp => xdo.mouse_up(param1),
                    MouseCommand::MoveMouseRelative => xdo.move_mouse_relative(param1, param2),
                    MouseCommand::Shutdown => unreachable!(),
                };
                if let Err(e) = result {
                    log::warn!("Mouse command failed: {:?}", e);
                }
            } else if let Err(e) = execute_ydotool("ydotool", command, param1, param2) {
                log::warn!("ydotool mouse command failed: {}", e);
            }
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
        MouseCommand::MouseDown => {
            process.args(["click", "--", "0x40"]);
        }
        MouseCommand::MouseUp => {
            process.args(["click", "--", "0x80"]);
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

    pub fn move_mouse_relative(&mut self, x_val: i32, y_val: i32) {
        if x_val == 0 && y_val == 0 {
            return;
        }

        self.cancel_timer_if_present();
        if let Some(ref tx) = self.tx {
            match tx.try_send((MouseCommand::MoveMouseRelative, x_val, y_val)) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    self.dropped_move_events = self.dropped_move_events.saturating_add(1);
                    self.maybe_report_drop_stats();
                }
                Err(TrySendError::Disconnected(_)) => {
                    log::warn!("Mouse worker disconnected, dropping move event");
                }
            }
        }
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
