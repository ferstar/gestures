mod config;
mod gestures;
mod utils;
mod xdo_handler;

#[cfg(test)]
mod tests;

use std::{path::PathBuf, rc::Rc};

use anyhow::Result;
use clap::Parser;
use env_logger::Builder;
use log::LevelFilter;

use crate::config::*;
use crate::xdo_handler::start_handler;

fn main() -> Result<()> {
    let app = App::parse();

    {
        let mut l = Builder::from_default_env();

        if app.verbose > 0 {
            l.filter_level(match app.verbose {
                1 => LevelFilter::Info,
                2 => LevelFilter::Debug,
                _ => LevelFilter::max(),
            });
        }

        if app.debug {
            l.filter_level(LevelFilter::Debug);
        }

        l.init();
    }

    let c = if let Some(p) = app.conf {
        Config::read_from_file(&p)?
    } else {
        config::Config::read_default_config().unwrap_or_else(|_| {
            log::error!("Could not read configuration file, using empty config!");
            Config::default()
        })
    };
    log::debug!("{:#?}", &c);
    let mut eh = gestures::EventHandler::new(Rc::new(c));
    let mut interface = input::Libinput::new_with_udev(gestures::Interface);
    eh.init(&mut interface)?;
    eh.main_loop(&mut interface, &mut start_handler(!app.wayland_disp));
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
