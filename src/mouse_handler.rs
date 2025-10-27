use chrono::Duration;
use libxdo::XDo;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use timer::Timer;
use std::env;
use std::path::Path;

#[derive(Copy, Clone)]
pub enum MouseCommand {
    MouseUp,
    MouseDown,
    MoveMouseRelative,
}

pub struct MouseHandler {
    tx: Option<mpsc::Sender<(MouseCommand, i32, i32)>>,
    timer: Timer,
    guard: Option<timer::Guard>,
    handler_mouse_down: bool,
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

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            // Try to initialize XDo, fallback to None if it fails
            let xdo_result = XDo::new(None);

            match xdo_result {
                Ok(xdo) => {
                    log::info!("Successfully initialized libxdo for X11");
                    while let Ok((command, param1, param2)) = rx.recv() {
                        let _ = match command {
                            MouseCommand::MouseDown => xdo.mouse_down(param1),
                            MouseCommand::MouseUp => xdo.mouse_up(param1),
                            MouseCommand::MoveMouseRelative => xdo.move_mouse_relative(param1, param2),
                        };
                    }
                },
                Err(e) => {
                    log::error!("Failed to initialize libxdo: {:?}", e);
                    log::warn!("X11 mouse control will not work. Consider:");
                    log::warn!("  1. Running with --wayland flag to use ydotool instead");
                    log::warn!("  2. Checking DISPLAY and XAUTHORITY environment variables");
                    log::warn!("  3. Installing ydotool for Wayland support");
                    // Don't panic, just exit the thread - will fallback to ydotool mode
                }
            }
        });
        Some(tx)
    } else {
        None
    };

    MouseHandler {
        tx,
        timer: Timer::new(),
        guard: None,
        handler_mouse_down: false,
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
        self.handler_mouse_down = true;
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
        self.handler_mouse_down = false;
    }

    pub fn move_mouse_relative(&mut self, x_val: i32, y_val: i32) {
        self.cancel_timer_if_present();
        if let Some(ref tx) = self.tx {
            let _ = tx.send((MouseCommand::MoveMouseRelative, x_val, y_val));
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
            self.handler_mouse_down = true;
        }
    }
}
