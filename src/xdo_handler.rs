use chrono::Duration;
use libxdo::XDo;
use std::sync::mpsc;
use std::thread;
use timer::Timer;

// 1. 使用 #[derive(Copy, Clone)] 对简单枚举优化
#[derive(Copy, Clone)]
pub enum XDoCommand {
    MouseUp,
    MouseDown,
    MoveMouseRelative,
}

pub struct XDoHandler {
    tx: mpsc::Sender<(XDoCommand, i32, i32)>,
    timer: Timer,
    guard: Option<timer::Guard>,
    handler_mouse_down: bool,
    pub is_xorg: bool,
}

pub fn start_handler(is_xorg: bool) -> XDoHandler {
    let (tx, rx) = mpsc::channel();
    let timer = Timer::new();
    
    if is_xorg {
        thread::spawn(move || {
            // 2. 将 XDo 实例移到线程外部以避免重复创建
            let xdo = XDo::new(None).expect("can not initialize libxdo");
            
            // 3. 使用 while let 替代 loop + match 模式，更符合 Rust 习惯
            while let Ok((command, param1, param2)) = rx.recv() {
                // 4. 使用 let _ = 处理 Result，避免 unwrap
                let _ = match command {
                    XDoCommand::MouseDown => xdo.mouse_down(param1),
                    XDoCommand::MouseUp => xdo.mouse_up(param1),
                    XDoCommand::MoveMouseRelative => xdo.move_mouse_relative(param1, param2),
                };
            }
        });
    }

    XDoHandler {
        tx,
        timer,
        guard: None,
        handler_mouse_down: false,
        is_xorg,
    }
}

impl XDoHandler {
    // 5. 使用 '&mut self' 而不是移动所有权
    pub fn mouse_down(&mut self, button: i32) {
        self.cancel_timer_if_present();
        let _ = self.tx.send((XDoCommand::MouseDown, button, 255));
        self.handler_mouse_down = true;
    }

    pub fn mouse_up_delay(&mut self, button: i32, delay_ms: i64) {
        let tx_clone = self.tx.clone();
        self.guard = Some(self.timer.schedule_with_delay(
            Duration::milliseconds(delay_ms),
            move || {
                let _ = tx_clone.send((XDoCommand::MouseUp, button, 255));
            },
        ));
        self.handler_mouse_down = false;
    }

    pub fn move_mouse_relative(&mut self, x_val: i32, y_val: i32) {
        self.cancel_timer_if_present();
        let _ = self.tx.send((XDoCommand::MoveMouseRelative, x_val, y_val));
    }

    fn cancel_timer_if_present(&mut self) {
        if self.guard.is_some() {
            self.guard = None;
            self.handler_mouse_down = true;
        }
    }
}
