use chrono::Duration;
use libxdo::XDo;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use timer::Timer;

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

pub fn start_handler(is_xorg: bool) -> MouseHandler {
    let tx = if is_xorg {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let xdo = XDo::new(None).expect("can not initialize libxdo");

            while let Ok((command, param1, param2)) = rx.recv() {
                let _ = match command {
                    MouseCommand::MouseDown => xdo.mouse_down(param1),
                    MouseCommand::MouseUp => xdo.mouse_up(param1),
                    MouseCommand::MoveMouseRelative => xdo.move_mouse_relative(param1, param2),
                };
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
