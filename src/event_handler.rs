use std::{
    fs::{File, OpenOptions},
    os::{
        fd::OwnedFd,
        unix::prelude::{AsRawFd, OpenOptionsExt},
    },
    path::Path,
    sync::{Arc, RwLock},
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
use libc::{O_RDWR, O_WRONLY, O_RDONLY};
use miette::{miette, Result};
use nix::poll::{poll, PollFd, PollFlags};
// use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::gestures::{hold::*, pinch::*, swipe::*, *};
use crate::xdo_handler::XDoHandler;
use crate::utils::exec_command_from_string;

#[derive(Debug)]
pub struct EventHandler {
    config: Arc<RwLock<Config>>,
    event: Gesture,
}

impl EventHandler {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        Self {
            config,
            event: Gesture::None,
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
        let mut found = false;
        log::debug!("Looking for gesture device");
        input.dispatch().unwrap();
        for event in input.clone() {
            if let Event::Device(e) = event {
                log::debug!("Device: {:?}", &e);
                found = e.device().has_capability(DeviceCapability::Gesture);
                log::debug!("Supports gestures: {:?}", found);
                if found {
                    return found;
                }
            } else {
                continue;
            }
            input.dispatch().unwrap();
        }
        found
    }

    pub fn main_loop(&mut self, input: &mut Libinput, xdoh: &mut XDoHandler) {
        let fds = PollFd::new(input.as_raw_fd(), PollFlags::POLLIN);
        while poll(&mut [fds], -1).is_ok() {
            self.handle_event(input, xdoh)
                .expect("An Error occurred while handling an event");
        }
    }

    pub fn handle_event(&mut self, input: &mut Libinput, xdoh: &mut XDoHandler) -> Result<()> {
        input.dispatch().unwrap();
        for event in input.clone() {
            if let Event::Gesture(e) = event {
                match e {
                    GestureEvent::Pinch(e) => self.handle_pinch_event(e)?,
                    GestureEvent::Swipe(e) => self.handle_swipe_event(e, xdoh)?,
                    GestureEvent::Hold(e) => self.handle_hold_event(e)?,
                    _ => (),
                }
            }
            input.dispatch().unwrap();
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
                    for i in &self.config.clone().read().unwrap().gestures {
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
                    for i in &self.config.clone().read().unwrap().gestures {
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
                    for i in &self.config.clone().read().unwrap().gestures {
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
                    for i in &self.config.clone().read().unwrap().gestures {
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
            GestureSwipeEvent::Begin(e) => {
                self.event = Gesture::Swipe(Swipe {
                    direction: SwipeDir::Any,
                    fingers: e.finger_count(),
                    update: None,
                    start: None,
                    end: None,
                    acceleration: None,
                    mouse_up_delay: None,
                });
                if let Gesture::Swipe(s) = &self.event {
                    for gesture in &self.config.clone().read().unwrap().gestures {
                        if let Gesture::Swipe(j) = gesture {
                            if j.fingers == s.fingers {
                                let is_xorg_condition = xdoh.is_xorg
                                    && j.acceleration.is_some()
                                    && j.mouse_up_delay.is_some()
                                    && j.direction == SwipeDir::Any;
                                if is_xorg_condition {
                                    log::debug!("Call libxdo api directly in Xorg env for better performance.");
                                    xdoh.mouse_down(1);
                                } else if j.direction == s.direction || j.direction == SwipeDir::Any
                                {
                                    exec_command_from_string(
                                        &j.start.as_ref().unwrap_or(&String::new()),
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
            GestureSwipeEvent::Update(e) => {
                let (x, y) = (e.dx(), e.dy());
                let swipe_dir = SwipeDir::dir(x, y);

                if let Gesture::Swipe(s) = &self.event {
                    log::debug!("{:?}  {:?}", &swipe_dir, &s.fingers);
                    for gesture in &self.config.clone().read().unwrap().gestures {
                        if let Gesture::Swipe(j) = gesture {
                            if j.fingers == s.fingers {
                                let is_xorg_condition = xdoh.is_xorg
                                    && j.acceleration.is_some()
                                    && j.mouse_up_delay.is_some()
                                    && j.direction == SwipeDir::Any;
                                if is_xorg_condition {
                                    let x_val =
                                        x * j.acceleration.unwrap_or_default() as f64 / 10.0;
                                    let y_val =
                                        y * j.acceleration.unwrap_or_default() as f64 / 10.0;
                                    xdoh.move_mouse_relative(x_val as i32, y_val as i32);
                                } else if j.direction == swipe_dir || j.direction == SwipeDir::Any {
                                    exec_command_from_string(
                                        &j.update.as_ref().unwrap_or(&String::new()),
                                        x,
                                        y,
                                        0.0,
                                        0.0,
                                    )?;
                                }
                            }
                        }
                    }
                    self.event = Gesture::Swipe(Swipe {
                        direction: swipe_dir,
                        fingers: s.fingers,
                        update: None,
                        start: None,
                        end: None,
                        acceleration: None,
                        mouse_up_delay: None,
                    })
                }
            }
            GestureSwipeEvent::End(e) => {
                if let Gesture::Swipe(s) = &self.event {
                    if !e.cancelled() {
                        for gesture in &self.config.clone().read().unwrap().gestures {
                            if let Gesture::Swipe(j) = gesture {
                                if j.fingers == s.fingers {
                                    let is_xorg_condition = xdoh.is_xorg
                                        && j.acceleration.is_some()
                                        && j.mouse_up_delay.is_some()
                                        && j.direction == SwipeDir::Any;
                                    if is_xorg_condition {
                                        xdoh.mouse_up_delay(
                                            1,
                                            j.mouse_up_delay.clone().unwrap_or_default(),
                                        );
                                    } else if j.direction == s.direction
                                        || j.direction == SwipeDir::Any
                                    {
                                        exec_command_from_string(
                                            &j.end.as_ref().unwrap_or(&String::new()),
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
            }
            _ => (),
        }
        Ok(())
    }
}

pub struct Interface;

impl LibinputInterface for Interface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
        let read_flag = (flags & O_RDONLY != 0) || (flags & O_RDWR != 0);
        let write_flag = (flags & O_WRONLY != 0) || (flags & O_RDWR != 0);

        OpenOptions::new()
            .custom_flags(flags)
            .read(read_flag)
            .write(write_flag)
            .open(path)
            .map(OwnedFd::from)
            .map_err(|err| err.raw_os_error().unwrap())
    }
    fn close_restricted(&mut self, fd: OwnedFd) {
        let _ = File::from(fd);
    }
}
