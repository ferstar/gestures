use chrono::Duration;
use libxdo::XDo;
use std::env;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::sync::mpsc::TrySendError;
use std::thread;
use std::time::Duration as StdDuration;
use std::time::Instant;
use timer::Timer;

fn current_uid() -> Option<u32> {
    std::fs::metadata("/proc/self").ok().map(|m| m.uid())
}

#[derive(Copy, Clone)]
pub enum MouseCommand {
    MouseUp,
    MouseDown,
    MoveMouseRelative,
}

pub struct MouseHandler {
    tx: Option<mpsc::SyncSender<(MouseCommand, i32, i32)>>,
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

pub fn start_handler(is_xorg: bool) -> MouseHandler {
    let tx = if is_xorg {
        // Setup X11 environment before initializing XDo
        setup_x11_env();

        const MOUSE_EVENT_QUEUE_SIZE: usize = 64;
        let (tx, rx) = mpsc::sync_channel(MOUSE_EVENT_QUEUE_SIZE);
        let (ready_tx, ready_rx) = mpsc::channel();

        thread::spawn(move || match XDo::new(None) {
            Ok(xdo) => {
                let _ = ready_tx.send(Ok(()));
                log::info!("Successfully initialized libxdo for X11");
                let mut pending: Option<(MouseCommand, i32, i32)> = None;
                let mut coalesced_move_events: u64 = 0;
                let mut last_coalesce_report = Instant::now();
                loop {
                    let (command, mut param1, mut param2) = if let Some(cmd) = pending.take() {
                        cmd
                    } else {
                        match rx.recv() {
                            Ok(cmd) => cmd,
                            Err(_) => break,
                        }
                    };

                    if matches!(command, MouseCommand::MoveMouseRelative) {
                        while let Ok((next_cmd, next_p1, next_p2)) = rx.try_recv() {
                            match next_cmd {
                                MouseCommand::MoveMouseRelative => {
                                    param1 = param1.saturating_add(next_p1);
                                    param2 = param2.saturating_add(next_p2);
                                    coalesced_move_events = coalesced_move_events.saturating_add(1);
                                }
                                _ => {
                                    pending = Some((next_cmd, next_p1, next_p2));
                                    break;
                                }
                            }
                        }
                    }

                    let _ = match command {
                        MouseCommand::MouseDown => xdo.mouse_down(param1),
                        MouseCommand::MouseUp => xdo.mouse_up(param1),
                        MouseCommand::MoveMouseRelative => xdo.move_mouse_relative(param1, param2),
                    };

                    if log::log_enabled!(log::Level::Debug)
                        && coalesced_move_events > 0
                        && last_coalesce_report.elapsed() >= StdDuration::from_secs(10)
                    {
                        log::debug!(
                            "x11 move queue stats: coalesced_move_events={}",
                            coalesced_move_events
                        );
                        coalesced_move_events = 0;
                        last_coalesce_report = Instant::now();
                    }
                }
            }
            Err(e) => {
                let _ = ready_tx.send(Err(e));
            }
        });

        match ready_rx.recv_timeout(StdDuration::from_secs(2)) {
            Ok(Ok(())) => Some(tx),
            Ok(Err(e)) => {
                log::error!("Failed to initialize libxdo: {:?}", e);
                log::warn!("Falling back to ydotool mouse control mode");
                log::warn!("Check DISPLAY/XAUTHORITY if X11 mode was intended");
                None
            }
            Err(e) => {
                log::error!("Timed out waiting for libxdo initialization: {:?}", e);
                log::warn!("Falling back to ydotool mouse control mode");
                None
            }
        }
    } else {
        None
    };

    MouseHandler {
        tx,
        timer: Timer::new(),
        guard: None,
        dropped_move_events: 0,
        last_drop_report: Instant::now(),
    }
}

impl MouseHandler {
    pub fn mouse_down(&mut self, button: i32) {
        self.cancel_timer_if_present();
        if let Some(ref tx) = self.tx {
            let _ = tx.send((MouseCommand::MouseDown, button, 255));
        } else {
            let _ = Command::new("ydotool")
                .args(["click", "--", "0x40"])
                .spawn();
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
        } else {
            self.guard = Some(self.timer.schedule_with_delay(
                Duration::milliseconds(delay_ms),
                move || {
                    let _ = Command::new("ydotool")
                        .args(["click", "--", "0x80"])
                        .spawn();
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
        } else {
            let _ = Command::new("ydotool")
                .args([
                    "mousemove",
                    "-x",
                    &x_val.to_string(),
                    "-y",
                    &y_val.to_string(),
                ])
                .spawn();
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
                "x11 move queue stats: dropped_move_events={}",
                self.dropped_move_events
            );
            self.dropped_move_events = 0;
            self.last_drop_report = Instant::now();
        }
    }
}
