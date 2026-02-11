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
        Event, EventTrait, GestureEvent,
    },
    DeviceCapability, Libinput, LibinputInterface,
};
use miette::{miette, Result};
use nix::{
    fcntl::OFlag,
    poll::{poll, PollFd, PollFlags, PollTimeout},
};

use crate::config::Config;
use crate::gestures::{hold::*, pinch::*, swipe::*, *};
use crate::mouse_handler::MouseHandler;
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
}

impl EventHandler {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        let mut handler = Self {
            config,
            event: Gesture::None,
            cache: GestureCache::new(),
            throttle: ThrottleState::new(60),
        };
        handler.update_cache();
        handler
    }

    pub fn init(&mut self, input: &mut Libinput) -> Result<()> {
        log::debug!("{:?}  {:?}", &self, &input);
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
                log::debug!("Device: {:?}", &e);
                if e.device().has_capability(DeviceCapability::Gesture) {
                    log::debug!("Found gesture device");
                    return true;
                }
            }
        }

        log::debug!("No gesture device found");
        false
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
        for event in input {
            if let Event::Gesture(e) = event {
                match e {
                    GestureEvent::Pinch(e) => self.handle_pinch_event(e)?,
                    GestureEvent::Swipe(e) => self.handle_swipe_event(e, mh)?,
                    GestureEvent::Hold(e) => self.handle_hold_event(e)?,
                    _ => (),
                }
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
                    log::debug!("Hold: {:?}", &s.fingers);
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
                        &scale,
                        &delta_angle,
                        &dir,
                        &s.fingers
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
        mh: &mut MouseHandler,
    ) -> Result<()> {
        match event {
            GestureSwipeEvent::Begin(e) => self.handle_swipe_begin(e.finger_count(), mh),
            GestureSwipeEvent::Update(e) => self.handle_swipe_update(e.dx(), e.dy(), mh),
            GestureSwipeEvent::End(e) => {
                if !e.cancelled() {
                    self.handle_swipe_end(mh)
                } else {
                    Ok(())
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
        mh: &mut MouseHandler,
        handler: F,
    ) -> Result<()>
    where
        F: Fn(&Gesture, &mut MouseHandler) -> Result<()>,
    {
        self.refresh_cache_if_needed();

        if let Gesture::Swipe(_) = &self.event {
            if let Some(gestures) = self.cache.swipe_gestures.get(&fingers) {
                for gesture in gestures {
                    handler(gesture, mh)?;
                }
            }
        }
        Ok(())
    }

    fn is_direct_mouse_gesture(gesture: &Gesture) -> bool {
        if let Gesture::Swipe(j) = gesture {
            j.acceleration.is_some() && j.mouse_up_delay.is_some() && j.direction == SwipeDir::Any
        } else {
            false
        }
    }

    fn handle_swipe_begin(&mut self, fingers: i32, mh: &mut MouseHandler) -> Result<()> {
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

    fn handle_swipe_update(&mut self, dx: f64, dy: f64, mh: &mut MouseHandler) -> Result<()> {
        let swipe_dir = SwipeDir::dir(dx, dy);
        let (fingers, current_dir) = if let Gesture::Swipe(s) = &self.event {
            (s.fingers, swipe_dir.clone())
        } else {
            return Ok(());
        };

        log::debug!("{:?} {:?}", &current_dir, &fingers);

        let is_throttled = !self.throttle.should_update();

        let current_dir = current_dir.clone();
        self.handle_matching_gesture(fingers, mh, move |gesture, mh| {
            if let Gesture::Swipe(j) = gesture {
                if Self::is_direct_mouse_gesture(gesture) {
                    if !is_throttled {
                        let acceleration = j.acceleration.unwrap_or_default() as f64 / 10.0;
                        mh.move_mouse_relative(
                            (dx * acceleration) as i32,
                            (dy * acceleration) as i32,
                        );
                    }
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

        self.event = Gesture::Swipe(Swipe::with_direction(fingers, swipe_dir));
        Ok(())
    }

    fn handle_swipe_end(&mut self, mh: &mut MouseHandler) -> Result<()> {
        let (fingers, direction) = if let Gesture::Swipe(s) = &self.event {
            (s.fingers, s.direction.clone())
        } else {
            return Ok(());
        };
        self.handle_matching_gesture(fingers, mh, |gesture, mh| {
            if let Gesture::Swipe(j) = gesture {
                if Self::is_direct_mouse_gesture(gesture) {
                    let delay = j.mouse_up_delay.unwrap_or_default();
                    mh.mouse_up_delay(1, delay);
                } else if j.direction == direction || j.direction == SwipeDir::Any {
                    exec_command_from_string(j.end.as_deref().unwrap_or(""), 0.0, 0.0, 0.0, 0.0)?;
                }
            }
            Ok(())
        })
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
