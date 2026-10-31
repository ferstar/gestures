use input::{AccelProfile, Device, DeviceCapability, DeviceConfigError};

use crate::config::{TouchpadAccelProfile, TouchpadConfig};

pub(crate) trait TouchpadDevice {
    fn name(&self) -> &str;
    fn has_gestures(&self) -> bool;
    fn has_acceleration(&self) -> bool;
    fn set_profile(&mut self, profile: AccelProfile) -> Result<(), DeviceConfigError>;
    fn set_speed(&mut self, speed: f64) -> Result<(), DeviceConfigError>;
}

impl TouchpadDevice for Device {
    fn name(&self) -> &str {
        Device::name(self)
    }

    fn has_gestures(&self) -> bool {
        self.has_capability(DeviceCapability::Gesture)
    }

    fn has_acceleration(&self) -> bool {
        self.config_accel_is_available()
    }

    fn set_profile(&mut self, profile: AccelProfile) -> Result<(), DeviceConfigError> {
        self.config_accel_set_profile(profile)
    }

    fn set_speed(&mut self, speed: f64) -> Result<(), DeviceConfigError> {
        self.config_accel_set_speed(speed)
    }
}

fn apply_config(device: &mut impl TouchpadDevice, config: TouchpadConfig) {
    if config == TouchpadConfig::default() {
        return;
    }
    if !device.has_acceleration() {
        log::debug!(
            "Gesture device '{}' has no pointer acceleration",
            device.name()
        );
        return;
    }
    if let Some(profile) = config.accel_profile {
        let profile = match profile {
            TouchpadAccelProfile::Adaptive => AccelProfile::Adaptive,
            TouchpadAccelProfile::Flat => AccelProfile::Flat,
        };
        if let Err(error) = device.set_profile(profile) {
            log::warn!(
                "Cannot set touchpad accel-profile {profile:?} on '{}': {error:?}",
                device.name()
            );
        }
    }
    if let Some(speed) = config.accel_speed {
        if let Err(error) = device.set_speed(speed.0) {
            log::warn!(
                "Cannot set touchpad accel-speed {} on '{}': {error:?}",
                speed.0,
                device.name()
            );
        }
    }
}

#[derive(Debug)]
pub(crate) struct TouchpadDevices<D> {
    devices: Vec<D>,
    config: TouchpadConfig,
}

impl<D: TouchpadDevice + PartialEq> TouchpadDevices<D> {
    pub fn new(config: TouchpadConfig) -> Self {
        Self {
            devices: Vec::new(),
            config,
        }
    }

    pub fn add(&mut self, mut device: D) {
        if device.has_gestures() && !self.devices.contains(&device) {
            apply_config(&mut device, self.config);
            self.devices.push(device);
        }
    }

    pub fn remove(&mut self, device: &D) {
        self.devices.retain(|existing| existing != device);
    }

    pub fn has_devices(&self) -> bool {
        !self.devices.is_empty()
    }

    pub fn update_config(&mut self, config: TouchpadConfig) {
        if self.config != config {
            for device in &mut self.devices {
                apply_config(device, config);
            }
            self.config = config;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AccelSpeed;
    use std::{cell::RefCell, rc::Rc};

    #[derive(Debug, PartialEq)]
    enum Call {
        Profile(AccelProfile),
        Speed(f64),
    }

    #[derive(Debug, Clone)]
    struct MockDevice {
        id: u32,
        gestures: bool,
        acceleration: bool,
        fail_profile: bool,
        calls: Rc<RefCell<Vec<Call>>>,
    }

    impl PartialEq for MockDevice {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }

    impl MockDevice {
        fn new(id: u32) -> Self {
            Self {
                id,
                gestures: true,
                acceleration: true,
                fail_profile: false,
                calls: Rc::new(RefCell::new(Vec::new())),
            }
        }
    }

    impl TouchpadDevice for MockDevice {
        fn name(&self) -> &str {
            "mock touchpad"
        }

        fn has_gestures(&self) -> bool {
            self.gestures
        }

        fn has_acceleration(&self) -> bool {
            self.acceleration
        }

        fn set_profile(&mut self, profile: AccelProfile) -> Result<(), DeviceConfigError> {
            self.calls.borrow_mut().push(Call::Profile(profile));
            if self.fail_profile {
                Err(DeviceConfigError::Unsupported)
            } else {
                Ok(())
            }
        }

        fn set_speed(&mut self, speed: f64) -> Result<(), DeviceConfigError> {
            self.calls.borrow_mut().push(Call::Speed(speed));
            Ok(())
        }
    }

    #[test]
    fn only_sets_explicitly_configured_fields() {
        for (config, expected) in [
            (TouchpadConfig::default(), vec![]),
            (
                TouchpadConfig {
                    accel_profile: Some(TouchpadAccelProfile::Adaptive),
                    accel_speed: None,
                },
                vec![Call::Profile(AccelProfile::Adaptive)],
            ),
            (
                TouchpadConfig {
                    accel_profile: None,
                    accel_speed: Some(AccelSpeed(0.3)),
                },
                vec![Call::Speed(0.3)],
            ),
        ] {
            let mut device = MockDevice::new(1);
            apply_config(&mut device, config);
            assert_eq!(*device.calls.borrow(), expected);
        }
    }

    #[test]
    fn reload_updates_existing_devices_and_hotplug_uses_current_config() {
        let mut devices = TouchpadDevices::new(TouchpadConfig::default());
        let first = MockDevice::new(1);
        devices.add(first.clone());
        assert!(first.calls.borrow().is_empty());
        let config = TouchpadConfig {
            accel_profile: Some(TouchpadAccelProfile::Flat),
            accel_speed: Some(AccelSpeed(-0.4)),
        };
        devices.update_config(config);
        devices.update_config(config);
        assert_eq!(
            *first.calls.borrow(),
            [Call::Profile(AccelProfile::Flat), Call::Speed(-0.4)]
        );
        let second = MockDevice::new(2);
        devices.add(second.clone());
        devices.add(second.clone());
        assert_eq!(*second.calls.borrow(), *first.calls.borrow());
        devices.remove(&first);
        let next = TouchpadConfig {
            accel_profile: None,
            accel_speed: Some(AccelSpeed(0.8)),
        };
        devices.update_config(next);
        assert_eq!(first.calls.borrow().len(), 2);
        assert_eq!(second.calls.borrow().last(), Some(&Call::Speed(0.8)));
        devices.remove(&second);
        assert!(!devices.has_devices());
    }

    #[test]
    fn unsupported_devices_are_skipped_and_profile_failure_does_not_block_speed() {
        let config = TouchpadConfig {
            accel_profile: Some(TouchpadAccelProfile::Flat),
            accel_speed: Some(AccelSpeed(0.5)),
        };
        let mut devices = TouchpadDevices::new(config);
        let mut no_gestures = MockDevice::new(1);
        no_gestures.gestures = false;
        devices.add(no_gestures.clone());
        assert!(!devices.has_devices());
        assert!(no_gestures.calls.borrow().is_empty());
        let mut no_acceleration = MockDevice::new(2);
        no_acceleration.acceleration = false;
        devices.add(no_acceleration.clone());
        assert!(no_acceleration.calls.borrow().is_empty());
        let mut rejected_profile = MockDevice::new(3);
        rejected_profile.fail_profile = true;
        devices.add(rejected_profile.clone());
        assert_eq!(
            *rejected_profile.calls.borrow(),
            [Call::Profile(AccelProfile::Flat), Call::Speed(0.5)]
        );
    }
}
