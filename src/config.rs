use std::{env, fs, path::Path};

use miette::{miette, IntoDiagnostic, Result};
// use serde::{Deserialize, Serialize};
use knuffel::{
    ast::{Literal, TypeName},
    decode::Context,
    errors::DecodeError,
    parse,
    span::Spanned,
    traits::ErrorSpan,
    Decode, DecodeScalar,
};

use crate::gestures::Gesture;

#[derive(Decode, PartialEq, Debug, Default)]
pub struct Config {
    #[knuffel(child)]
    pub touchpad: Option<TouchpadConfig>,
    #[knuffel(children)]
    pub gestures: Vec<Gesture>,
}

#[derive(Decode, PartialEq, Debug, Default, Clone, Copy)]
pub struct TouchpadConfig {
    #[knuffel(property)]
    pub accel_profile: Option<TouchpadAccelProfile>,
    #[knuffel(property)]
    pub accel_speed: Option<AccelSpeed>,
}

#[derive(DecodeScalar, PartialEq, Debug, Clone, Copy)]
pub enum TouchpadAccelProfile {
    Adaptive,
    Flat,
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct AccelSpeed(pub f64);

impl<S: ErrorSpan> DecodeScalar<S> for AccelSpeed {
    fn type_check(type_name: &Option<Spanned<TypeName, S>>, ctx: &mut Context<S>) {
        f64::type_check(type_name, ctx);
    }

    fn raw_decode(
        value: &Spanned<Literal, S>,
        _ctx: &mut Context<S>,
    ) -> std::result::Result<Self, DecodeError<S>> {
        let invalid = || {
            DecodeError::conversion(
                value,
                "touchpad accel-speed must be a finite number in [-1.0, 1.0]",
            )
        };
        let speed = match &**value {
            Literal::Int(integer) => i64::try_from(integer).map_err(|_| invalid())? as f64,
            Literal::Decimal(decimal) => f64::try_from(decimal).map_err(|_| invalid())?,
            _ => return Err(invalid()),
        };
        if !(-1.0..=1.0).contains(&speed) {
            return Err(invalid());
        }
        Ok(Self(speed))
    }
}

impl Config {
    pub fn read_from_file(file: &Path) -> Result<Self> {
        log::debug!("{:?}", file);
        match fs::read_to_string(file) {
            Ok(s) => {
                let source_name = file.to_string_lossy();
                Ok(parse::<Config>(&source_name, &s).into_diagnostic()?)
            }
            Err(_) => Err(miette!("Could not read config file")),
        }
    }

    pub fn get_config_home() -> Result<String> {
        if let Ok(config_home) = env::var("XDG_CONFIG_HOME") {
            return Ok(config_home);
        }

        let home = env::var("HOME").map_err(|_| {
            miette::miette!("Both XDG_CONFIG_HOME and HOME environment variables are unset")
        })?;
        Ok(format!("{home}/.config"))
    }

    pub fn read_default_config() -> Result<Self> {
        let config_home = Self::get_config_home()?;

        log::debug!("{:?}", config_home);

        for path in ["gestures.kdl", "gestures/gestures.kdl"] {
            match Self::read_from_file(Path::new(&format!("{config_home}/{path}"))) {
                Ok(s) => return Ok(s),
                Err(e) => log::warn!("{}", e),
            }
        }

        Err(miette!("Could not find config file"))
    }

    pub fn read_from_optional_path(path: Option<&Path>) -> Result<Self> {
        if let Some(path) = path {
            Self::read_from_file(path)
        } else {
            Self::read_default_config()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touchpad_node_is_optional_and_coexists_with_all_gestures() {
        let gestures = r#"
swipe fingers=3 direction="any" mouse-up-delay=500
pinch fingers=2 direction="in" update="zoom $scale"
hold fingers=4 action="launcher"
"#;
        let original: Config = parse("test.kdl", gestures).unwrap();
        assert_eq!(original.touchpad, None);
        for source in [
            format!("touchpad accel-profile=\"adaptive\" accel-speed=0.3\n{gestures}"),
            format!("{gestures}\ntouchpad accel-profile=\"adaptive\" accel-speed=0.3\n"),
        ] {
            let configured: Config = parse("test.kdl", &source).unwrap();
            assert_eq!(configured.gestures, original.gestures);
            assert_eq!(
                configured.touchpad,
                Some(TouchpadConfig {
                    accel_profile: Some(TouchpadAccelProfile::Adaptive),
                    accel_speed: Some(AccelSpeed(0.3)),
                })
            );
        }
    }

    #[test]
    fn touchpad_properties_can_be_configured_independently() {
        for (source, expected) in [
            ("touchpad\n", TouchpadConfig::default()),
            (
                "touchpad accel-profile=\"flat\"\n",
                TouchpadConfig {
                    accel_profile: Some(TouchpadAccelProfile::Flat),
                    accel_speed: None,
                },
            ),
            (
                "touchpad accel-speed=-0.25\n",
                TouchpadConfig {
                    accel_profile: None,
                    accel_speed: Some(AccelSpeed(-0.25)),
                },
            ),
        ] {
            let config: Config = parse("test.kdl", source).unwrap();
            assert_eq!(config.touchpad, Some(expected));
        }
        for (literal, speed) in [("-1.0", -1.0), ("0", 0.0), ("1.0", 1.0)] {
            let config: Config =
                parse("test.kdl", &format!("touchpad accel-speed={literal}\n")).unwrap();
            assert_eq!(
                config.touchpad.unwrap().accel_speed,
                Some(AccelSpeed(speed))
            );
        }
    }

    #[test]
    fn rejects_invalid_touchpad_speed_with_clear_error() {
        for speed in ["-1.01", "1.01", "1e999", "-1e999"] {
            let error = parse::<Config>("test.kdl", &format!("touchpad accel-speed={speed}\n"))
                .unwrap_err();
            assert!(
                format!("{error:?}")
                    .contains("touchpad accel-speed must be a finite number in [-1.0, 1.0]"),
                "{error:?}"
            );
        }
        assert!(parse::<Config>("test.kdl", "touchpad accel-speed=\"fast\"\n").is_err());
    }

    #[test]
    fn rejects_unknown_profiles_and_duplicate_touchpad_nodes() {
        assert!(parse::<Config>("test.kdl", "touchpad accel-profile=\"unknown\"\n").is_err());
        assert!(parse::<Config>("test.kdl", "touchpad\ntouchpad\n").is_err());
    }
}
