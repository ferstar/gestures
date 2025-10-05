mod config;
mod event_handler;
mod gestures;
mod ipc;
mod ipc_client;
mod utils;
mod xdo_handler;

#[cfg(test)]
mod tests;

use parking_lot::RwLock;
use std::{
    path::PathBuf,
    sync::{
        atomic::AtomicBool,
        Arc, LazyLock,
    },
    thread::{self, JoinHandle},
};

use clap::{Parser, Subcommand};
use env_logger::Builder;
use log::LevelFilter;
use miette::Result;

use crate::config::*;
use crate::xdo_handler::start_handler;

pub static SHUTDOWN: LazyLock<Arc<AtomicBool>> = LazyLock::new(|| Arc::new(AtomicBool::new(false)));

fn main() -> Result<()> {
    let app = App::parse();

    // Setup signal handlers for graceful shutdown
    signal_hook::flag::register(signal_hook::consts::SIGTERM, SHUTDOWN.clone())
        .expect("Failed to register SIGTERM handler");
    signal_hook::flag::register(signal_hook::consts::SIGINT, SHUTDOWN.clone())
        .expect("Failed to register SIGINT handler");

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

    match app.command {
        c @ Commands::Reload => {
            ipc_client::handle_command(c);
        }
        Commands::Start => run_eh(Arc::new(RwLock::new(c)), app.wayland_disp)?,
    }

    Ok(())
}

fn run_eh(config: Arc<RwLock<Config>>, is_wayland: bool) -> Result<()> {
    let eh_thread = spawn_event_handler(config.clone(), is_wayland);
    ipc::create_socket(config);
    eh_thread.join().unwrap()?;
    Ok(())
}

fn spawn_event_handler(config: Arc<RwLock<Config>>, is_wayland: bool) -> JoinHandle<Result<()>> {
    thread::spawn(move || {
        log::debug!("Starting event handler in new thread");
        let mut eh = event_handler::EventHandler::new(config);
        let mut interface = input::Libinput::new_with_udev(event_handler::Interface);
        eh.init(&mut interface)?;
        let _ = eh.main_loop(&mut interface, &mut start_handler(!is_wayland));
        Ok(())
    })
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
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Reload the configuration
    Reload,
    /// Start the program
    Start,
}
