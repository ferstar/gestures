use std::{
    fs::{File, OpenOptions},
    os::unix::prelude::{AsRawFd, FromRawFd, IntoRawFd, OpenOptionsExt, RawFd},
    path::Path,
    rc::Rc,
};

use anyhow::{bail, Result};
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
use libc::{O_RDWR, O_WRONLY};
use nix::poll::{poll, PollFd, PollFlags};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::utils::exec_command_from_string;

/// Direction of swipe gestures
///
/// NW  N  NE
/// W   C   E
/// SW  S  SE
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    #[serde(rename = "any")]
    Any,
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

impl Direction {
    // This code is sort of a mess
    pub fn dir(x: f64, y: f64) -> Direction {
        if x.abs() == 0.0 && y.abs() == 0.0 {
            return Direction::Any;
        }
        let oblique_ratio = 0.414;
        if x.abs() > y.abs() {
            let sd = if x < 0.0 { Direction::W } else { Direction::E };
            if y.abs() / x.abs() > oblique_ratio {
                if sd == Direction::W {
                    if y < 0.0 {
                        Direction::NW
                    } else {
                        Direction::SW
                    }
                } else if sd == Direction::E {
                    if y < 0.0 {
                        Direction::NE
                    } else {
                        Direction::SE
                    }
                } else {
                    Direction::Any
                }
            } else {
                sd
            }
        } else {
            // Don't ask me why, but for libinput the coordinates increase downward. This does
            // hold out the same as screen coordinates, but it starts in the center instead of
            // the upper left. I have also noticed game controllers work the same way.
            let sd = if y < 0.0 { Direction::N } else { Direction::S };
            if x.abs() / y.abs() > oblique_ratio {
                if sd == Direction::N {
                    if x < 0.0 {
                        Direction::NW
                    } else {
                        Direction::NE
                    }
                } else if sd == Direction::S {
                    if x < 0.0 {
                        Direction::SW
                    } else {
                        Direction::SE
                    }
                } else {
                    Direction::Any
                }
            } else {
                sd
            }
        }
    }
}

/// Direction of pinch gestures
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InOut {
    In,
    Out,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Repeat {
    Oneshot,
    Continuous,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gesture {
    Swipe(Swipe),
    Pinch(Pinch),
    Hold(Hold),
    Rotate(Rotate),
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Swipe {
    pub direction: Direction,
    pub fingers: i32,
    pub update: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pinch {
    pub fingers: i32,
    pub direction: InOut,
    pub update: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hold {
    pub fingers: i32,
    pub action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rotate {
    pub scale: f64,
    pub fingers: i32,
    pub delta_angle: f64,
    pub repeat: Repeat,
    pub action: String,
}

#[derive(Debug)]
pub struct EventHandler {
    config: Rc<Config>,
    event: Gesture,
}

impl EventHandler {
    pub fn new(config: Rc<Config>) -> Self {
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
            bail!("Could not find gesture device");
        }
    }

    fn init_ctx(&mut self, input: &mut Libinput) -> Result<(), ()> {
        input.udev_assign_seat("seat0")?;
        Ok(())
    }

    fn has_gesture_device(&mut self, input: &mut Libinput) -> bool {
        let mut found = false;
        input.dispatch().unwrap();
        for event in input.clone() {
            if let Event::Device(e) = event {
                log::debug!("{:?}", &e);
                found = e.device().has_capability(DeviceCapability::Gesture);
                log::debug!("{:?}", found);
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

    pub fn main_loop(&mut self, input: &mut Libinput) {
        let fds = PollFd::new(input.as_raw_fd(), PollFlags::POLLIN);
        while poll(&mut [fds], -1).is_ok() {
            self.handle_event(input)
                .expect("An Error occurred while handling an event");
        }
    }

    pub fn handle_event(&mut self, input: &mut Libinput) -> Result<()> {
        input.dispatch().unwrap();
        for event in input.clone() {
            if let Event::Gesture(e) = event {
                match e {
                    GestureEvent::Pinch(e) => self.handle_pinch_event(e)?,
                    GestureEvent::Swipe(e) => self.handle_swipe_event(e)?,
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
                    log::debug!("Hold  {:?}", &s.fingers);
                    for i in &self.config.clone().gestures {
                        if let Gesture::Hold(j) = i {
                            if j.fingers == s.fingers {
                                exec_command_from_string(
                                    &j.action.clone().unwrap_or_default(),
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
                    direction: InOut::Any,
                    update: None,
                    start: None,
                    end: None,
                });
                if let Gesture::Pinch(s) = &self.event {
                    for i in &self.config.clone().gestures {
                        if let Gesture::Pinch(j) = i {
                            if (j.direction == s.direction || j.direction == InOut::Any)
                                && j.fingers == s.fingers
                            {
                                log::debug!("oneshot pinch gesture");
                                exec_command_from_string(
                                    &j.start.clone().unwrap_or_default(),
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
                if let Gesture::Pinch(s) = &self.event {
                    let dir = if scale > 1.0 { InOut::Out } else { InOut::In };
                    log::debug!("{:?}  {:?}  {:?}", &scale, &dir, &s.fingers);
                    for i in &self.config.clone().gestures {
                        if let Gesture::Pinch(j) = i {
                            if (j.direction == dir || j.direction == InOut::Any)
                                && j.fingers == s.fingers
                            {
                                exec_command_from_string(
                                    &j.update.clone().unwrap_or_default(),
                                    0.0,
                                    0.0,
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
                    for i in &self.config.clone().gestures {
                        if let Gesture::Pinch(j) = i {
                            if (j.direction == s.direction || j.direction == InOut::Any)
                                && j.fingers == s.fingers
                            {
                                exec_command_from_string(
                                    &j.end.clone().unwrap_or_default(),
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

    fn handle_swipe_event(&mut self, event: GestureSwipeEvent) -> Result<()> {
        match event {
            GestureSwipeEvent::Begin(e) => {
                self.event = Gesture::Swipe(Swipe {
                    direction: Direction::Any,
                    fingers: e.finger_count(),
                    update: None,
                    start: None,
                    end: None,
                });
                if let Gesture::Swipe(s) = &self.event {
                    for i in &self.config.clone().gestures {
                        if let Gesture::Swipe(j) = i {
                            if j.fingers == s.fingers
                                && (j.direction == s.direction || j.direction == Direction::Any)
                            {
                                exec_command_from_string(
                                    &j.start.clone().unwrap_or_default(),
                                    0.0,
                                    0.0,
                                    0.0,
                                )?;
                            }
                        }
                    }
                }
            }
            GestureSwipeEvent::Update(e) => {
                let (x, y) = (e.dx(), e.dy());
                let swipe_dir = Direction::dir(x, y);

                if let Gesture::Swipe(s) = &self.event {
                    log::debug!("{:?}  {:?}", &swipe_dir, &s.fingers);
                    for i in &self.config.clone().gestures {
                        if let Gesture::Swipe(j) = i {
                            if j.fingers == s.fingers
                                && (j.direction == swipe_dir || j.direction == Direction::Any)
                            {
                                exec_command_from_string(
                                    &j.update.clone().unwrap_or_default(),
                                    x,
                                    y,
                                    0.0,
                                )?;
                            }
                        }
                    }
                    self.event = Gesture::Swipe(Swipe {
                        direction: swipe_dir,
                        fingers: s.fingers,
                        update: None,
                        start: None,
                        end: None,
                    })
                }
            }
            GestureSwipeEvent::End(e) => {
                if let Gesture::Swipe(s) = &self.event {
                    if !e.cancelled() {
                        for i in &self.config.clone().gestures {
                            if let Gesture::Swipe(j) = i {
                                if j.fingers == s.fingers
                                    && (j.direction == s.direction || j.direction == Direction::Any)
                                {
                                    exec_command_from_string(
                                        &j.end.clone().unwrap_or_default(),
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
}

pub struct Interface;

impl LibinputInterface for Interface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<RawFd, i32> {
        OpenOptions::new()
            .custom_flags(flags)
            .read((false) | (flags & O_RDWR != 0))
            .write((flags & O_WRONLY != 0) | (flags & O_RDWR != 0))
            .open(path)
            .map(|file| file.into_raw_fd())
            .map_err(|err| err.raw_os_error().unwrap())
    }
    fn close_restricted(&mut self, fd: RawFd) {
        unsafe {
            File::from_raw_fd(fd);
        }
    }
}
