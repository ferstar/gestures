use chrono::Duration;
use libxdo::XDo;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use timer::Timer;

#[derive(Copy, Clone)]
pub enum XDoCommand {
    MouseUp,
    MouseDown,
    MoveMouseRelative,
}

pub struct XDoHandler {
    tx: Option<mpsc::Sender<(XDoCommand, i32, i32)>>,
    timer: Timer,
    guard: Option<timer::Guard>,
    handler_mouse_down: bool,
}

pub fn start_handler(is_xorg: bool) -> XDoHandler {
    let tx = if is_xorg {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let xdo = XDo::new(None).expect("can not initialize libxdo");

            while let Ok((command, param1, param2)) = rx.recv() {
                let _ = match command {
                    XDoCommand::MouseDown => xdo.mouse_down(param1),
                    XDoCommand::MouseUp => xdo.mouse_up(param1),
                    XDoCommand::MoveMouseRelative => xdo.move_mouse_relative(param1, param2),
                };
            }
        });
        Some(tx)
    } else {
        None
    };

    XDoHandler {
        tx,
        timer: Timer::new(),
        guard: None,
        handler_mouse_down: false,
    }
}

impl XDoHandler {
    pub fn mouse_down(&mut self, button: i32) {
        self.cancel_timer_if_present();
        if let Some(ref tx) = self.tx {
            let _ = tx.send((XDoCommand::MouseDown, button, 255));
        } else {
            // Wayland: use ydotool
            let _ = Command::new("ydotool")
                .args(&["click", "--", "0x40"])
                .spawn();
        }
        self.handler_mouse_down = true;
    }

    pub fn mouse_up_delay(&mut self, button: i32, delay_ms: i64) {
        if let Some(ref tx) = self.tx {
            // X11: send via channel
            let tx_clone = tx.clone();
            self.guard = Some(self.timer.schedule_with_delay(
                Duration::milliseconds(delay_ms),
                move || {
                    let _ = tx_clone.send((XDoCommand::MouseUp, button, 255));
                },
            ));
        } else {
            // Wayland: schedule ydotool command
            self.guard = Some(self.timer.schedule_with_delay(
                Duration::milliseconds(delay_ms),
                move || {
                    let _ = Command::new("ydotool")
                        .args(&["click", "--", "0x80"])
                        .spawn();
                },
            ));
        }
        self.handler_mouse_down = false;
    }

    pub fn move_mouse_relative(&mut self, x_val: i32, y_val: i32) {
        self.cancel_timer_if_present();
        if let Some(ref tx) = self.tx {
            let _ = tx.send((XDoCommand::MoveMouseRelative, x_val, y_val));
        } else {
            // Wayland: use ydotool
            let _ = Command::new("ydotool")
                .args(&["mousemove", "-x", &x_val.to_string(), "-y", &y_val.to_string()])
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
