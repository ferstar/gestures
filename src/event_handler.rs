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
use crate::utils::exec_command_from_string;
use crate::xdo_handler::XDoHandler;

use std::collections::HashMap;
use parking_lot::RwLock;

// Add cache struct
#[derive(Debug)]
struct GestureCache {
    swipe_gestures: HashMap<i32, Vec<Gesture>>,
    last_update: std::time::Instant,
}

impl GestureCache {
    fn new() -> Self {
        Self {
            swipe_gestures: HashMap::new(),
            last_update: std::time::Instant::now(),
        }
    }
}

// Throttle state for wayland swipe updates
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
    config: Arc<RwLock<Config>>, // Changed from std::sync::RwLock
    event: Gesture,
    cache: GestureCache,
    throttle: ThrottleState,
}

impl EventHandler {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        Self {
            config,
            event: Gesture::None,
            cache: GestureCache::new(),
            throttle: ThrottleState::new(120), // 120 FPS limit for wayland updates
        }
    }

    pub fn init(&mut self, input: &mut Libinput) -> Result<()> {
        log::debug!("{:?}  {:?}", &self, &input);
        self.init_ctx(input).expect("Could not initialize libinput");
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

    pub fn main_loop(&mut self, input: &mut Libinput, xdoh: &mut XDoHandler) -> Result<()> {
        loop {
            // Check shutdown flag
            if crate::SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed) {
                log::info!("Received shutdown signal, exiting event loop");
                break;
            }

            let mut fds = [PollFd::new(input.as_fd(), PollFlags::POLLIN)];
            // Use timeout instead of NONE to allow checking shutdown flag
            match poll(&mut fds, PollTimeout::try_from(100).unwrap()) {
                Ok(_) => {
                    self.handle_event(input, xdoh)?;
                }
                Err(e) => {
                    // Only break if it's not an interrupt
                    if e != nix::errno::Errno::EINTR {
                        return Err(miette!("Poll error: {}", e));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn handle_event(&mut self, input: &mut Libinput, xdoh: &mut XDoHandler) -> Result<()> {
        input.dispatch().unwrap();
        for event in input {
            if let Event::Gesture(e) = event {
                match e {
                    GestureEvent::Pinch(e) => self.handle_pinch_event(e)?,
                    GestureEvent::Swipe(e) => self.handle_swipe_event(e, xdoh)?,
                    GestureEvent::Hold(e) => self.handle_hold_event(e)?,
                    _ => (),
                }
            }
        }
        Ok(())
    }

    fn handle_hold_event(&mut self, event: GestureHoldEvent) -> Result<()> {
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
                    for i in &self.config.clone().read().gestures {
                        if let Gesture::Hold(j) = i {
                            if j.fingers == s.fingers {
                                exec_command_from_string(
                                    &j.action.clone().unwrap_or_default(),
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
                    for i in &self.config.clone().read().gestures {
                        if let Gesture::Pinch(j) = i {
                            if (j.direction == s.direction || j.direction == PinchDir::Any)
                                && j.fingers == s.fingers
                            {
                                exec_command_from_string(
                                    &j.start.clone().unwrap_or_default(),
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
            GesturePinchEvent::Update(e) => {
                let scale = e.scale();
                let delta_angle = e.angle_delta();
                if let Gesture::Pinch(s) = &self.event {
                    let dir = PinchDir::dir(scale, delta_angle);
                    log::debug!(
                        "Pinch: scale={:?} angle={:?} direction={:?} fingers={:?}",
                        &scale,
                        &delta_angle,
                        &dir,
                        &s.fingers
                    );
                    for i in &self.config.clone().read().gestures {
                        if let Gesture::Pinch(j) = i {
                            if (j.direction == dir || j.direction == PinchDir::Any)
                                && j.fingers == s.fingers
                            {
                                exec_command_from_string(
                                    &j.update.clone().unwrap_or_default(),
                                    0.0,
                                    0.0,
                                    delta_angle,
                                    scale,
                                )?;
                            }
                        }
                    }
                    self.event = Gesture::Pinch(Pinch {
                        fingers: s.fingers,
                        direction: dir,
                        update: None,
                        start: None,
                        end: None,
                    })
                }
            }
            GesturePinchEvent::End(_e) => {
                if let Gesture::Pinch(s) = &self.event {
                    for i in &self.config.clone().read().gestures {
                        if let Gesture::Pinch(j) = i {
                            if (j.direction == s.direction || j.direction == PinchDir::Any)
                                && j.fingers == s.fingers
                            {
                                exec_command_from_string(
                                    &j.end.clone().unwrap_or_default(),
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

    fn handle_swipe_event(
        &mut self,
        event: GestureSwipeEvent,
        xdoh: &mut XDoHandler,
    ) -> Result<()> {
        match event {
            GestureSwipeEvent::Begin(e) => self.handle_swipe_begin(e.finger_count(), xdoh),
            GestureSwipeEvent::Update(e) => self.handle_swipe_update(e.dx(), e.dy(), xdoh),
            GestureSwipeEvent::End(e) => {
                if !e.cancelled() {
                    self.handle_swipe_end(xdoh)
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }

    fn update_cache(&mut self) {
        let config = self.config.read(); // No need for unwrap()
        let mut swipe_map: HashMap<i32, Vec<Gesture>> = HashMap::new();

        for gesture in &config.gestures {
            if let Gesture::Swipe(swipe) = gesture {
                swipe_map
                    .entry(swipe.fingers)
                    .or_default()
                    .push(gesture.clone());
            }
        }

        self.cache.swipe_gestures = swipe_map;
        self.cache.last_update = std::time::Instant::now();
    }

    fn handle_matching_gesture<F>(
        &mut self,
        fingers: i32,
        xdoh: &mut XDoHandler,
        handler: F,
    ) -> Result<()>
    where
        F: Fn(&Gesture, &mut XDoHandler) -> Result<()>,
    {
        // Update cache if needed
        if self.cache.last_update.elapsed() > std::time::Duration::from_secs(1) {
            self.update_cache();
        }

        if let Gesture::Swipe(_) = &self.event {
            if let Some(gestures) = self.cache.swipe_gestures.get(&fingers) {
                for gesture in gestures {
                    handler(gesture, xdoh)?;
                }
            }
        }
        Ok(())
    }

    fn is_xorg_gesture(gesture: &Gesture, xdoh: &XDoHandler) -> bool {
        if let Gesture::Swipe(j) = gesture {
            xdoh.is_xorg
                && j.acceleration.is_some()
                && j.mouse_up_delay.is_some()
                && j.direction == SwipeDir::Any
        } else {
            false
        }
    }

    fn handle_swipe_begin(&mut self, fingers: i32, xdoh: &mut XDoHandler) -> Result<()> {
        self.event = Gesture::Swipe(Swipe::new(fingers));

        self.handle_matching_gesture(fingers, xdoh, |gesture, xdoh| {
            if Self::is_xorg_gesture(gesture, xdoh) {
                log::debug!("Call libxdo api directly in Xorg env for better performance.");
                xdoh.mouse_down(1);
            } else if let Gesture::Swipe(j) = gesture {
                if j.direction == SwipeDir::Any {
                    exec_command_from_string(j.start.as_deref().unwrap_or(""), 0.0, 0.0, 0.0, 0.0)?;
                }
            }
            Ok(())
        })
    }

    fn handle_swipe_update(&mut self, dx: f64, dy: f64, xdoh: &mut XDoHandler) -> Result<()> {
        let swipe_dir = SwipeDir::dir(dx, dy);
        let (fingers, current_dir) = if let Gesture::Swipe(s) = &self.event {
            (s.fingers, swipe_dir.clone())
        } else {
            return Ok(());
        };

        log::debug!("{:?} {:?}", &current_dir, &fingers);

        // Check throttle before processing
        let should_throttle_update = !self.throttle.should_update();

        let current_dir = current_dir.clone();
        self.handle_matching_gesture(fingers, xdoh, move |gesture, xdoh| {
            if let Gesture::Swipe(j) = gesture {
                if Self::is_xorg_gesture(gesture, xdoh) {
                    let acceleration = j.acceleration.unwrap_or_default() as f64 / 10.0;
                    xdoh.move_mouse_relative(
                        (dx * acceleration) as i32,
                        (dy * acceleration) as i32,
                    );
                } else if (j.direction == current_dir || j.direction == SwipeDir::Any) && !should_throttle_update {
                    // Throttle wayland command execution
                    exec_command_from_string(j.update.as_deref().unwrap_or(""), dx, dy, 0.0, 0.0)?;
                }
            }
            Ok(())
        })?;

        self.event = Gesture::Swipe(Swipe::with_direction(fingers, swipe_dir));
        Ok(())
    }

    fn handle_swipe_end(&mut self, xdoh: &mut XDoHandler) -> Result<()> {
        let (fingers, direction) = if let Gesture::Swipe(s) = &self.event {
            (s.fingers, s.direction.clone())
        } else {
            return Ok(());
        };
        self.handle_matching_gesture(fingers, xdoh, |gesture, xdoh| {
            if let Gesture::Swipe(j) = gesture {
                if Self::is_xorg_gesture(gesture, xdoh) {
                    xdoh.mouse_up_delay(1, j.mouse_up_delay.unwrap_or_default());
                } else if j.direction == direction || j.direction == SwipeDir::Any {
                    exec_command_from_string(
                        j.end.as_deref().unwrap_or(""),
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                    )?;
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
