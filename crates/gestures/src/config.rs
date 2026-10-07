use std::{env, fs, path::Path};

use miette::{bail, IntoDiagnostic, Result};
// use serde::{Deserialize, Serialize};
use knuffel::{parse, Decode};

use crate::gestures::Gesture;

#[derive(Decode, PartialEq, Debug, Default)]
pub struct Config {
    // pub device: Option<String>,
    #[knuffel(children)]
    pub gestures: Vec<Gesture>,
}

impl Config {
    pub fn read_from_file(file: &Path) -> Result<Self> {
        log::debug!("{:?}", &file);
        match fs::read_to_string(file) {
            Ok(s) => {
                let source_name = file.to_string_lossy();
                Ok(parse::<Config>(&source_name, &s).into_diagnostic()?)
            }
            _ => bail!("Could not read config file"),
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

        log::debug!("{:?}", &config_home);

        for path in ["gestures.kdl", "gestures/gestures.kdl"] {
            match Self::read_from_file(Path::new(&format!("{config_home}/{path}"))) {
                Ok(s) => return Ok(s),
                Err(e) => log::warn!("{}", e),
            }
        }

        bail!("Could not find config file")
    }

    pub fn read_from_optional_path(path: Option<&Path>) -> Result<Self> {
        if let Some(path) = path {
            Self::read_from_file(path)
        } else {
            Self::read_default_config()
        }
    }
}
