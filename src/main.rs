mod config;
mod gestures;
mod utils;
mod xdo_handler;

#[cfg(test)]
mod tests;

use std::{path::PathBuf, rc::Rc};

use clap::Parser;

use crate::config::*;

fn main() {
    let app = App::parse();

    let c = config::Config::read_default_config().unwrap_or_else(|_| {
        eprintln!("Could not read configuration file, using empty config!");
        Config::default()
    });
    let debug = app.debug || app.verbose > 0;
    if_debug!(debug, &c);
    let mut eh = gestures::EventHandler::new(Rc::new(c));
    let mut interface = input::Libinput::new_with_udev(gestures::Interface);
    eh.init(&mut interface)
        .expect("could not initialize libinput");
    eh.main_loop(&mut interface);
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
    /// Path to config file
    #[arg(short, long, value_name = "FILE")]
    conf: Option<PathBuf>,
}
