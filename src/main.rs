mod config;
mod gestures;
mod utils;
mod xdo_handler;

#[cfg(test)]
mod tests;

use std::{path::PathBuf, rc::Rc};

use clap::Parser;
use env_logger::Builder;
use log::LevelFilter;
use miette::Result;

use crate::config::*;
use crate::xdo_handler::start_handler;

fn main() -> Result<()> {
    let app = App::parse();

    // Set up logging based on verbosity and debug flags
    let mut logger = Builder::from_default_env();
    if app.verbose > 0 {
        logger.filter_level(match app.verbose {
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::max(),
        });
    }
    if app.debug {
        logger.filter_level(LevelFilter::Debug);
    }
    logger.init();

    // Load configuration from file or use default
    let config = app.conf
        .map_or_else(|| config::Config::read_default_config(), |p| Config::read_from_file(&p))
        .unwrap_or_else(|_| {
            log::error!("Could not read configuration file, using empty config!");
            Config::default()
        });

    log::debug!("{:#?}", &config);

    let mut event_handler = gestures::EventHandler::new(Rc::new(config));
    let mut interface = input::Libinput::new_with_udev(gestures::Interface);
    event_handler.init(&mut interface)?;
    event_handler.main_loop(&mut interface, &mut start_handler(!app.wayland_disp));
    Ok(())
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct App {
    /// Verbosity, can be repeated
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    /// Debug mode
    #[arg(short, long)]
    debug: bool,
    /// Is Wayland desktop env or not
    /// (default: Xorg, will use xdotool api directly for better 3-finger-drag performance)
    #[arg(short, long)]
    wayland_disp: bool,
    /// Path to config file
    #[arg(short, long, value_name = "FILE")]
    conf: Option<PathBuf>,
}
