use std::{
    fs::OpenOptions,
    os::{
        fd::{AsFd, OwnedFd},
        unix::prelude::OpenOptionsExt,
    },
    path::Path,
    sync::Arc,
};

use input::{
    event::{
        gesture::{
            GestureEndEvent, GestureEventCoordinates, GestureEventTrait, GestureHoldEvent,
            GesturePinchEvent, GesturePinchEventTrait, GestureSwipeEvent,
        },
        DeviceEvent, Event, EventTrait, GestureEvent,
    },
    Device, Libinput, LibinputInterface,
};
use miette::{miette, Result};
use nix::{
    fcntl::OFlag,
    poll::{poll, PollFd, PollFlags, PollTimeout},
};

use crate::config::Config;
use crate::gestures::{hold::*, pinch::*, swipe::*, *};
use crate::mouse_handler::MouseHandler;
use crate::touchpad::TouchpadDevices;
use crate::utils::{exec_command_from_string, exec_update_command_from_string};

use parking_lot::RwLock;
use std::collections::HashMap;

#[derive(Debug)]
struct GestureCache {
    swipe_gestures: HashMap<i32, Vec<Gesture>>,
    pinch_gestures: HashMap<i32, Vec<Gesture>>,
    hold_gestures: HashMap<i32, Vec<Gesture>>,
    last_update: std::time::Instant,
}

impl GestureCache {
    fn new() -> Self {
        Self {
            swipe_gestures: HashMap::new(),
            pinch_gestures: HashMap::new(),
            hold_gestures: HashMap::new(),
            last_update: std::time::Instant::now() - std::time::Duration::from_secs(2),
        }
    }
}

/// Limits custom swipe update commands only; direct mouse dragging is unthrottled.
#[derive(Debug)]
struct ThrottleState {
    last_update: std::time::Instant,
    min_interval: std::time::Duration,
}

impl ThrottleState {
    fn new(fps: u32) -> Self {
        Self {
            last_update: std::time::Instant::now(),
            min_interval: std::time::Duration::from_micros(1_000_000 / fps as u64),
        }
    }

    fn should_update(&mut self) -> bool {
        let now = std::time::Instant::now();
        if now.duration_since(self.last_update) >= self.min_interval {
            self.last_update = now;
            true
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub struct EventHandler {
    config: Arc<RwLock<Config>>,
    event: Gesture,
    cache: GestureCache,
    throttle: ThrottleState,
    drag_motion: DragMotion,
    touchpads: TouchpadDevices<Device>,
}

#[derive(Debug, Default, Clone, Copy)]
struct DragMotion {
    x: f64,
    y: f64,
}

impl DragMotion {
    fn accumulate(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
    }

    fn flush(&mut self, mh: &mut dyn MouseActions) {
        let (x, y) = (self.x as i32, self.y as i32);
        if (x != 0 || y != 0) && mh.move_mouse_relative(x, y) {
            // Carry fractional pixels into the next update instead of discarding them.
            self.x -= x as f64;
            self.y -= y as f64;
        }
    }
}

trait MouseActions {
    fn mouse_down(&mut self, button: i32);
    fn mouse_up_delay(&mut self, button: i32, delay_ms: i64);
    fn move_mouse_relative(&mut self, x_val: i32, y_val: i32) -> bool;
}

impl MouseActions for MouseHandler {
    fn mouse_down(&mut self, button: i32) {
        MouseHandler::mouse_down(self, button);
    }

    fn mouse_up_delay(&mut self, button: i32, delay_ms: i64) {
        MouseHandler::mouse_up_delay(self, button, delay_ms);
    }

    fn move_mouse_relative(&mut self, x_val: i32, y_val: i32) -> bool {
        MouseHandler::move_mouse_relative(self, x_val, y_val)
    }
}

impl EventHandler {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        let touchpad_config = config.read().touchpad.unwrap_or_default();
        let mut handler = Self {
            config,
            event: Gesture::None,
            cache: GestureCache::new(),
            throttle: ThrottleState::new(60),
            drag_motion: DragMotion::default(),
            touchpads: TouchpadDevices::new(touchpad_config),
        };
        handler.update_cache();
        handler
    }

    pub fn init(&mut self, input: &mut Libinput) -> Result<()> {
        log::debug!("{:?}  {:?}", self, input);
        self.init_ctx(input)
            .map_err(|_| miette!("Could not initialize libinput"))?;
        if self.has_gesture_device(input) {
            Ok(())
        } else {
            Err(miette!("Could not find gesture device"))
        }
    }

    fn init_ctx(&mut self, input: &mut Libinput) -> Result<(), ()> {
        input.udev_assign_seat("seat0")?;
        Ok(())
    }

    fn has_gesture_device(&mut self, input: &mut Libinput) -> bool {
        log::debug!("Looking for gesture device");
        if let Err(e) = input.dispatch() {
            log::error!("Failed to dispatch input events: {}", e);
            return false;
        }

        for event in &mut *input {
            if let Event::Device(e) = event {
                self.handle_device_event(e);
            }
        }

        self.touchpads.has_devices()
    }

    fn handle_device_event(&mut self, event: DeviceEvent) {
        log::debug!("Device: {event:?}");
        match event {
            DeviceEvent::Added(event) => self.touchpads.add(event.device()),
            DeviceEvent::Removed(event) => self.touchpads.remove(&event.device()),
            _ => {}
        }
    }

    pub fn main_loop(&mut self, input: &mut Libinput, mh: &mut MouseHandler) -> Result<()> {
        loop {
            if crate::SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed) {
                log::info!("Received shutdown signal, exiting event loop");
                break;
            }

            let mut fds = [PollFd::new(input.as_fd(), PollFlags::POLLIN)];
            match poll(&mut fds, PollTimeout::from(100u16)) {
                Ok(_) => {
                    self.handle_event(input, mh)?;
                }
                Err(e) => {
                    if e != nix::errno::Errno::EINTR {
                        return Err(miette!("Poll error: {}", e));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn handle_event(&mut self, input: &mut Libinput, mh: &mut MouseHandler) -> Result<()> {
        input
            .dispatch()
            .map_err(|e| miette!("Failed to dispatch input events: {}", e))?;
        // Called even on poll timeouts, so IPC reload applies while devices are idle.
        let touchpad_config = self.config.read().touchpad.unwrap_or_default();
        self.touchpads.update_config(touchpad_config);
        for event in input {
            match event {
                Event::Device(event) => self.handle_device_event(event),
                Event::Gesture(e) => match e {
                    GestureEvent::Pinch(e) => self.handle_pinch_event(e)?,
                    GestureEvent::Swipe(e) => self.handle_swipe_event(e, mh)?,
                    GestureEvent::Hold(e) => self.handle_hold_event(e)?,
                    _ => (),
                },
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_hold_event(&mut self, event: GestureHoldEvent) -> Result<()> {
        self.refresh_cache_if_needed();
        match event {
            GestureHoldEvent::Begin(e) => {
                self.event = Gesture::Hold(Hold {
                    fingers: e.finger_count(),
                    action: None,
                })
            }
            GestureHoldEvent::End(_e) => {
                if let Gesture::Hold(s) = &self.event {
                    log::debug!("Hold: {:?}", s.fingers);
                    if let Some(gestures) = self.cache.hold_gestures.get(&s.fingers) {
                        for gesture in gestures {
                            if let Gesture::Hold(j) = gesture {
                                exec_command_from_string(
                                    j.action.as_deref().unwrap_or(""),
                                    0.0,
                                    0.0,
                                    0.0,
                                    0.0,
                                )?;
                            }
                        }
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }

    fn handle_pinch_event(&mut self, event: GesturePinchEvent) -> Result<()> {
        self.refresh_cache_if_needed();
        match event {
            GesturePinchEvent::Begin(e) => {
                self.event = Gesture::Pinch(Pinch {
                    fingers: e.finger_count(),
                    direction: PinchDir::Any,
                    update: None,
                    start: None,
                    end: None,
                });
                if let Gesture::Pinch(s) = &self.event {
                    if let Some(gestures) = self.cache.pinch_gestures.get(&s.fingers) {
                        for gesture in gestures {
                            if let Gesture::Pinch(j) = gesture {
                                if (j.direction == s.direction || j.direction == PinchDir::Any)
                                    && j.fingers == s.fingers
                                {
                                    exec_command_from_string(
                                        j.start.as_deref().unwrap_or(""),
                                        0.0,
                                        0.0,
                                        0.0,
                                        0.0,
                                    )?;
                                }
                            }
                        }
                    }
                }
            }
            GesturePinchEvent::Update(e) => {
                let scale = e.scale();
                let delta_angle = e.angle_delta();
                if let Gesture::Pinch(s) = &self.event {
                    let dir = PinchDir::dir(scale, delta_angle);
                    let fingers = s.fingers;
                    log::debug!(
                        "Pinch: scale={:?} angle={:?} direction={:?} fingers={:?}",
                        scale,
                        delta_angle,
                        dir,
                        s.fingers
                    );
                    if let Some(gestures) = self.cache.pinch_gestures.get(&fingers) {
                        for gesture in gestures {
                            if let Gesture::Pinch(j) = gesture {
                                if j.direction == dir || j.direction == PinchDir::Any {
                                    exec_update_command_from_string(
                                        j.update.as_deref().unwrap_or(""),
                                        0.0,
                                        0.0,
                                        delta_angle,
                                        scale,
                                    )?;
                                }
                            }
                        }
                    }
                    self.event = Gesture::Pinch(Pinch {
                        fingers,
                        direction: dir,
                        update: None,
                        start: None,
                        end: None,
                    })
                }
            }
            GesturePinchEvent::End(_e) => {
                if let Gesture::Pinch(s) = &self.event {
                    if let Some(gestures) = self.cache.pinch_gestures.get(&s.fingers) {
                        for gesture in gestures {
                            if let Gesture::Pinch(j) = gesture {
                                if (j.direction == s.direction || j.direction == PinchDir::Any)
                                    && j.fingers == s.fingers
                                {
                                    exec_command_from_string(
                                        j.end.as_deref().unwrap_or(""),
                                        0.0,
                                        0.0,
                                        0.0,
                                        0.0,
                                    )?;
                                }
                            }
                        }
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }

    fn handle_swipe_event(
        &mut self,
        event: GestureSwipeEvent,
        mh: &mut impl MouseActions,
    ) -> Result<()> {
        match event {
            GestureSwipeEvent::Begin(e) => self.handle_swipe_begin(e.finger_count(), mh),
            GestureSwipeEvent::Update(e) => self.handle_swipe_update(e.dx(), e.dy(), mh),
            GestureSwipeEvent::End(e) => {
                if e.cancelled() {
                    self.handle_swipe_cancel(mh)
                } else {
                    self.handle_swipe_end(mh)
                }
            }
            _ => Ok(()),
        }
    }

    fn update_cache(&mut self) {
        let config = self.config.read();
        let mut swipe_map: HashMap<i32, Vec<Gesture>> = HashMap::new();
        let mut pinch_map: HashMap<i32, Vec<Gesture>> = HashMap::new();
        let mut hold_map: HashMap<i32, Vec<Gesture>> = HashMap::new();

        for gesture in &config.gestures {
            match gesture {
                Gesture::Swipe(swipe) => {
                    if swipe.acceleration.is_some() {
                        log::warn!(
                            "swipe gesture property 'acceleration' is deprecated and ignored; use touchpad accel-speed / accel-profile instead"
                        );
                    }
                    swipe_map
                        .entry(swipe.fingers)
                        .or_default()
                        .push(gesture.clone());
                }
                Gesture::Pinch(pinch) => {
                    pinch_map
                        .entry(pinch.fingers)
                        .or_default()
                        .push(gesture.clone());
                }
                Gesture::Hold(hold) => {
                    hold_map
                        .entry(hold.fingers)
                        .or_default()
                        .push(gesture.clone());
                }
                Gesture::None => {}
            }
        }

        for (fingers, gestures) in &swipe_map {
            let direct: Vec<_> = gestures
                .iter()
                .filter(|gesture| Self::is_direct_mouse_gesture(gesture))
                .collect();
            let previous: Vec<_> = self
                .cache
                .swipe_gestures
                .get(fingers)
                .into_iter()
                .flatten()
                .filter(|gesture| Self::is_direct_mouse_gesture(gesture))
                .collect();
            if direct.len() > 1 && direct != previous {
                log::warn!(
                    "Multiple direct mouse gestures for {fingers} fingers; only the first is used"
                );
            }
        }
        self.cache.swipe_gestures = swipe_map;
        self.cache.pinch_gestures = pinch_map;
        self.cache.hold_gestures = hold_map;
        self.cache.last_update = std::time::Instant::now();
    }

    fn refresh_cache_if_needed(&mut self) {
        if self.cache.last_update.elapsed() > std::time::Duration::from_secs(1) {
            self.update_cache();
        }
    }

    fn handle_matching_gesture<F>(
        &mut self,
        fingers: i32,
        mh: &mut impl MouseActions,
        mut handler: F,
    ) -> Result<()>
    where
        F: FnMut(&Gesture, &mut dyn MouseActions) -> Result<()>,
    {
        self.refresh_cache_if_needed();

        if let Gesture::Swipe(_) = &self.event {
            if let Some(gestures) = self.cache.swipe_gestures.get(&fingers) {
                let mut handled_direct = false;
                for gesture in gestures {
                    if Self::is_direct_mouse_gesture(gesture) {
                        if handled_direct {
                            continue;
                        }
                        handled_direct = true;
                    }
                    handler(gesture, mh)?;
                }
            }
        }
        Ok(())
    }

    fn is_direct_mouse_gesture(gesture: &Gesture) -> bool {
        if let Gesture::Swipe(j) = gesture {
            j.mouse_up_delay.is_some() && j.direction == SwipeDir::Any
        } else {
            false
        }
    }

    fn handle_swipe_begin(&mut self, fingers: i32, mh: &mut impl MouseActions) -> Result<()> {
        self.drag_motion = DragMotion::default();
        self.throttle.last_update = std::time::Instant::now() - self.throttle.min_interval;
        self.event = Gesture::Swipe(Swipe::new(fingers));

        self.handle_matching_gesture(fingers, mh, |gesture, mh| {
            if Self::is_direct_mouse_gesture(gesture) {
                log::debug!("Using direct mouse control");
                mh.mouse_down(1);
            } else if let Gesture::Swipe(j) = gesture {
                if j.direction == SwipeDir::Any {
                    exec_command_from_string(j.start.as_deref().unwrap_or(""), 0.0, 0.0, 0.0, 0.0)?;
                }
            }
            Ok(())
        })
    }

    fn handle_swipe_update(&mut self, dx: f64, dy: f64, mh: &mut impl MouseActions) -> Result<()> {
        let swipe_dir = SwipeDir::dir(dx, dy);
        let (fingers, current_dir) = if let Gesture::Swipe(s) = &self.event {
            (s.fingers, swipe_dir.clone())
        } else {
            return Ok(());
        };

        log::debug!("{:?} {:?}", current_dir, fingers);

        let is_throttled = !self.throttle.should_update();
        let mut drag_motion = self.drag_motion;

        let current_dir = current_dir.clone();
        self.handle_matching_gesture(fingers, mh, |gesture, mh| {
            if let Gesture::Swipe(j) = gesture {
                if Self::is_direct_mouse_gesture(gesture) {
                    drag_motion.accumulate(dx, dy);
                    drag_motion.flush(mh);
                } else if (j.direction == current_dir || j.direction == SwipeDir::Any)
                    && !is_throttled
                {
                    exec_update_command_from_string(
                        j.update.as_deref().unwrap_or(""),
                        dx,
                        dy,
                        0.0,
                        0.0,
                    )?;
                }
            }
            Ok(())
        })?;
        self.drag_motion = drag_motion;

        self.event = Gesture::Swipe(Swipe::with_direction(fingers, swipe_dir));
        Ok(())
    }

    fn handle_swipe_end(&mut self, mh: &mut impl MouseActions) -> Result<()> {
        let (fingers, direction) = if let Gesture::Swipe(s) = &self.event {
            (s.fingers, s.direction.clone())
        } else {
            return Ok(());
        };
        let mut drag_motion = self.drag_motion;
        self.handle_matching_gesture(fingers, mh, |gesture, mh| {
            if let Gesture::Swipe(j) = gesture {
                if Self::is_direct_mouse_gesture(gesture) {
                    drag_motion.flush(mh);
                    let delay = j.mouse_up_delay.unwrap_or_default();
                    mh.mouse_up_delay(1, delay);
                } else if j.direction == direction || j.direction == SwipeDir::Any {
                    exec_command_from_string(j.end.as_deref().unwrap_or(""), 0.0, 0.0, 0.0, 0.0)?;
                }
            }
            Ok(())
        })?;
        self.drag_motion = DragMotion::default();
        self.event = Gesture::None;
        Ok(())
    }

    fn handle_swipe_cancel(&mut self, mh: &mut impl MouseActions) -> Result<()> {
        self.drag_motion = DragMotion::default();
        let fingers = if let Gesture::Swipe(s) = &self.event {
            s.fingers
        } else {
            return Ok(());
        };

        self.handle_matching_gesture(fingers, mh, |gesture, mh| {
            if Self::is_direct_mouse_gesture(gesture) {
                mh.mouse_up_delay(1, 0);
            }
            Ok(())
        })?;
        self.event = Gesture::None;
        Ok(())
    }
}

// Add this helper impl
impl Swipe {
    fn new(fingers: i32) -> Self {
        Self {
            direction: SwipeDir::Any,
            fingers,
            update: None,
            start: None,
            end: None,
            acceleration: None,
            mouse_up_delay: None,
        }
    }

    fn with_direction(fingers: i32, direction: SwipeDir) -> Self {
        Self {
            direction,
            fingers,
            update: None,
            start: None,
            end: None,
            acceleration: None,
            mouse_up_delay: None,
        }
    }
}

pub struct Interface;

impl LibinputInterface for Interface {
    #[inline]
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
        OpenOptions::new()
            .custom_flags(flags)
            .read(flags & OFlag::O_RDWR.bits() != 0)
            .write((flags & OFlag::O_WRONLY.bits() != 0) | (flags & OFlag::O_RDWR.bits() != 0))
            .open(path)
            .map(Into::into)
            .map_err(|err| err.raw_os_error().unwrap_or(-1))
    }

    #[inline]
    fn close_restricted(&mut self, fd: OwnedFd) {
        drop(fd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct MockMouseHandler {
        mouse_up_calls: Vec<(i32, i64)>,
        calls: Vec<MouseCall>,
        fail_next_move: bool,
    }

    #[derive(Debug, PartialEq)]
    enum MouseCall {
        Down,
        Move(i32, i32),
        Up,
    }

    impl MockMouseHandler {
        fn new() -> Self {
            Self {
                mouse_up_calls: Vec::new(),
                calls: Vec::new(),
                fail_next_move: false,
            }
        }
    }

    impl MouseActions for MockMouseHandler {
        fn mouse_down(&mut self, _button: i32) {
            self.calls.push(MouseCall::Down);
        }

        fn mouse_up_delay(&mut self, button: i32, delay_ms: i64) {
            self.mouse_up_calls.push((button, delay_ms));
            self.calls.push(MouseCall::Up);
        }

        fn move_mouse_relative(&mut self, x_val: i32, y_val: i32) -> bool {
            if std::mem::take(&mut self.fail_next_move) {
                return false;
            }
            self.calls.push(MouseCall::Move(x_val, y_val));
            true
        }
    }

    fn drag_handler() -> EventHandler {
        EventHandler::new(Arc::new(RwLock::new(Config {
            touchpad: None,
            gestures: vec![Gesture::Swipe(Swipe {
                direction: SwipeDir::Any,
                fingers: 3,
                update: None,
                start: None,
                end: None,
                acceleration: None,
                mouse_up_delay: Some(450),
            })],
        })))
    }

    fn throttle_updates(handler: &mut EventHandler) {
        handler.throttle.min_interval = std::time::Duration::from_secs(3600);
        handler.throttle.last_update = std::time::Instant::now();
    }

    fn allow_update(handler: &mut EventHandler) {
        handler.throttle.last_update = std::time::Instant::now() - handler.throttle.min_interval;
    }

    #[test]
    fn failed_move_is_retained_for_the_next_successful_update() {
        let mut handler = drag_handler();
        let mut mouse = MockMouseHandler::new();
        handler.handle_swipe_begin(3, &mut mouse).unwrap();
        mouse.fail_next_move = true;
        handler
            .handle_swipe_update(2.75, -1.25, &mut mouse)
            .unwrap();
        assert_eq!(mouse.calls, [MouseCall::Down]);
        assert_eq!(
            (handler.drag_motion.x, handler.drag_motion.y),
            (2.75, -1.25)
        );
        handler
            .handle_swipe_update(0.75, -0.75, &mut mouse)
            .unwrap();
        assert_eq!(mouse.calls, [MouseCall::Down, MouseCall::Move(3, -2)]);
        assert_eq!((handler.drag_motion.x, handler.drag_motion.y), (0.5, 0.0));
    }

    #[test]
    fn duplicate_direct_gestures_only_use_first_configuration() {
        for cancelled in [false, true] {
            let mut handler = drag_handler();
            {
                let mut config = handler.config.write();
                let mut duplicate = config.gestures[0].clone();
                if let Gesture::Swipe(swipe) = &mut duplicate {
                    swipe.mouse_up_delay = Some(999);
                }
                config.gestures.push(duplicate);
            }
            handler.update_cache();
            // Periodic refresh must preserve the same first-match behavior.
            handler.update_cache();
            let mut mouse = MockMouseHandler::new();
            handler.handle_swipe_begin(3, &mut mouse).unwrap();
            handler.handle_swipe_update(2.0, -1.0, &mut mouse).unwrap();
            if cancelled {
                handler.handle_swipe_cancel(&mut mouse).unwrap();
            } else {
                handler.handle_swipe_end(&mut mouse).unwrap();
            }
            assert_eq!(
                mouse.calls,
                [MouseCall::Down, MouseCall::Move(2, -1), MouseCall::Up]
            );
            assert_eq!(mouse.mouse_up_calls, [(1, if cancelled { 0 } else { 450 })]);
        }
    }

    #[test]
    fn direct_drag_emits_each_update_even_when_command_throttle_is_active() {
        let mut handler = drag_handler();
        let mut mouse = MockMouseHandler::new();
        handler.handle_swipe_begin(3, &mut mouse).unwrap();
        throttle_updates(&mut handler);
        handler.handle_swipe_update(3.25, -1.5, &mut mouse).unwrap();
        handler.handle_swipe_update(0.75, -0.5, &mut mouse).unwrap();
        handler.handle_swipe_update(2.0, -3.0, &mut mouse).unwrap();
        assert_eq!(
            mouse.calls,
            [
                MouseCall::Down,
                MouseCall::Move(3, -1),
                MouseCall::Move(1, -1),
                MouseCall::Move(2, -3)
            ]
        );
    }

    #[test]
    fn slow_drag_retains_fractional_pixels_between_frames() {
        let mut handler = drag_handler();
        let mut mouse = MockMouseHandler::new();
        handler.handle_swipe_begin(3, &mut mouse).unwrap();
        for _ in 0..4 {
            allow_update(&mut handler);
            handler
                .handle_swipe_update(0.25, -0.25, &mut mouse)
                .unwrap();
        }
        assert_eq!(mouse.calls, [MouseCall::Down, MouseCall::Move(1, -1)]);
    }

    #[test]
    fn ending_drag_flushes_pending_movement_before_releasing_button() {
        let mut handler = drag_handler();
        let mut mouse = MockMouseHandler::new();
        handler.handle_swipe_begin(3, &mut mouse).unwrap();
        throttle_updates(&mut handler);
        handler
            .handle_swipe_update(2.75, -1.25, &mut mouse)
            .unwrap();
        handler.handle_swipe_end(&mut mouse).unwrap();
        assert_eq!(
            mouse.calls,
            [MouseCall::Down, MouseCall::Move(2, -1), MouseCall::Up]
        );
        assert_eq!(mouse.mouse_up_calls, [(1, 450)]);
        assert_eq!((handler.drag_motion.x, handler.drag_motion.y), (0.0, 0.0));
    }

    #[test]
    fn cancelled_drag_does_not_carry_fractional_displacement() {
        let mut handler = drag_handler();
        let mut mouse = MockMouseHandler::new();
        handler.handle_swipe_begin(3, &mut mouse).unwrap();
        throttle_updates(&mut handler);
        handler
            .handle_swipe_update(0.25, -0.25, &mut mouse)
            .unwrap();
        handler.handle_swipe_cancel(&mut mouse).unwrap();
        handler.handle_swipe_begin(3, &mut mouse).unwrap();
        handler
            .handle_swipe_update(0.75, -0.75, &mut mouse)
            .unwrap();
        assert_eq!(
            mouse.calls,
            [MouseCall::Down, MouseCall::Up, MouseCall::Down]
        );
        handler
            .handle_swipe_update(0.25, -0.25, &mut mouse)
            .unwrap();
        assert_eq!(
            mouse.calls,
            [
                MouseCall::Down,
                MouseCall::Up,
                MouseCall::Down,
                MouseCall::Move(1, -1)
            ]
        );
    }

    #[test]
    fn drag_distance_is_independent_of_throttle_schedule() {
        let mut totals = Vec::new();
        for every in [1, 4, 1000] {
            let mut handler = drag_handler();
            let mut mouse = MockMouseHandler::new();
            handler.handle_swipe_begin(3, &mut mouse).unwrap();
            for i in 0..80 {
                throttle_updates(&mut handler);
                if i % every == 0 {
                    allow_update(&mut handler);
                }
                handler
                    .handle_swipe_update(0.375, -0.125, &mut mouse)
                    .unwrap();
            }
            handler.handle_swipe_end(&mut mouse).unwrap();
            let total = mouse.calls.iter().fold((0, 0), |(x, y), call| {
                if let MouseCall::Move(dx, dy) = call {
                    (x + dx, y + dy)
                } else {
                    (x, y)
                }
            });
            totals.push(total);
        }
        assert_eq!(totals, [(30, -10); 3]);
    }

    #[test]
    fn cancelled_swipe_releases_direct_mouse_drag() {
        let config = Config {
            touchpad: None,
            gestures: vec![Gesture::Swipe(Swipe {
                direction: SwipeDir::Any,
                fingers: 3,
                update: None,
                start: None,
                end: None,
                acceleration: None,
                mouse_up_delay: Some(500),
            })],
        };
        let mut handler = EventHandler::new(Arc::new(RwLock::new(config)));
        handler.event = Gesture::Swipe(Swipe::new(3));

        let mut mock_mouse = MockMouseHandler::new();
        handler
            .handle_swipe_cancel(&mut mock_mouse)
            .expect("cancelled swipe should be handled");

        assert_eq!(mock_mouse.mouse_up_calls, vec![(1, 0)]);
        assert_eq!(handler.event, Gesture::None);
    }
}
