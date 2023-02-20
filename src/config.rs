use std::{env, fs, path::Path};

use miette::{bail, IntoDiagnostic, Result};
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
            Ok(s) => Ok(parse::<Config>(file.to_str().unwrap(), &s).into_diagnostic()?),
            _ => bail!("Could not read config file"),
        }
    }

    pub fn read_default_config() -> Result<Self> {
        let config_home = env::var("XDG_CONFIG_HOME")
            .unwrap_or_else(|_| format!("{}/.config", env::var("HOME").unwrap()));

        log::debug!("{:?}", &config_home);

        for path in ["gestures.kdl", "gestures/gestures.kdl"] {
            match Self::read_from_file(Path::new(&format!("{config_home}/{path}"))) {
                Ok(s) => return Ok(s),
                Err(e) => log::warn!("{}", e),
            }
        }

        bail!("Could not find config file")
    }
}
