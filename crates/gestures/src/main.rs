mod config;
mod event_handler;
mod gestures;
mod ipc;
mod ipc_client;
mod mouse_handler;
mod touchpad;
mod utils;
mod xdo_dyn;

#[cfg(test)]
mod tests;

use parking_lot::RwLock;
use std::{
    env, fs,
    io::Write,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, LazyLock},
    thread::{self, JoinHandle},
};

use clap::{Parser, Subcommand};
use env_logger::Builder;
use log::LevelFilter;
use miette::Result;

use crate::config::*;
use crate::mouse_handler::start_handler;

pub static SHUTDOWN: LazyLock<Arc<AtomicBool>> = LazyLock::new(|| Arc::new(AtomicBool::new(false)));

/// Detect if running under Wayland by checking environment variables
fn detect_wayland() -> bool {
    // Check WAYLAND_DISPLAY first (most reliable indicator)
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return true;
    }

    // Check XDG_SESSION_TYPE as fallback
    if let Ok(session_type) = std::env::var("XDG_SESSION_TYPE") {
        return session_type.to_lowercase() == "wayland";
    }

    // Default to X11 if unable to detect
    false
}

/// Generate autostart desktop entry content
fn generate_autostart_desktop() -> Result<String> {
    let exe_path = env::current_exe()
        .map_err(|e| miette::miette!("Failed to get current executable path: {}", e))?;

    let exe_path_str = exe_path
        .to_str()
        .ok_or_else(|| miette::miette!("Executable path contains invalid UTF-8"))?;

    let content = format!(
        "[Desktop Entry]\nType=Application\nName=Gestures Daemon\nComment=Touchpad gestures daemon\nExec={} start\nTerminal=false\nStartupNotify=false\nX-GNOME-Autostart-enabled=true\n",
        exe_path_str
    );

    Ok(content)
}

/// Install or print autostart desktop file
fn install_autostart(print_only: bool) -> Result<()> {
    let content = generate_autostart_desktop()?;

    if print_only {
        print!("{}", content);
        return Ok(());
    }

    let home =
        env::var("HOME").map_err(|_| miette::miette!("HOME environment variable not set"))?;

    let autostart_dir = PathBuf::from(home).join(".config/autostart");
    let desktop_path = autostart_dir.join("gestures.desktop");

    fs::create_dir_all(&autostart_dir).map_err(|e| {
        miette::miette!(
            "Failed to create directory {}: {}",
            autostart_dir.display(),
            e
        )
    })?;

    let mut file = fs::File::create(&desktop_path).map_err(|e| {
        miette::miette!(
            "Failed to create desktop file {}: {}",
            desktop_path.display(),
            e
        )
    })?;

    file.write_all(content.as_bytes())
        .map_err(|e| miette::miette!("Failed to write desktop file: {}", e))?;

    println!(
        "✓ Autostart desktop file installed to: {}",
        desktop_path.display()
    );

    Ok(())
}

/// Generate default configuration content
fn get_default_config() -> &'static str {
    r#"// Gestures Configuration
// See https://github.com/ferstar/gestures for full documentation

// Optional libinput acceleration for this process only (does not change desktop settings):
// touchpad accel-profile="adaptive" accel-speed=0.3

// ====================
// 3-Finger Drag (macOS-like)
// ====================
// Works on both X11 and Wayland
// - X11: Uses libxdo API directly (minimal latency)
// - Wayland: Uses direct uinput writes; ydotool is a fallback
swipe direction="any" fingers=3 mouse-up-delay=500

// ====================
// 4-Finger Workspace Switching
// ====================
// Uncomment and adjust for your desktop environment:

// Hyprland:
// swipe direction="w" fingers=4 end="hyprctl dispatch workspace e-1"
// swipe direction="e" fingers=4 end="hyprctl dispatch workspace e+1"
// swipe direction="n" fingers=4 end="hyprctl dispatch fullscreen"
// swipe direction="s" fingers=4 end="hyprctl dispatch killactive"

// i3/Sway:
// swipe direction="w" fingers=4 end="i3-msg workspace prev"
// swipe direction="e" fingers=4 end="i3-msg workspace next"

// GNOME (requires gdbus):
// swipe direction="n" fingers=4 end="gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval global.workspace_manager.get_active_workspace().get_neighbor(Meta.MotionDirection.UP).activate(global.get_current_time())"

// ====================
// Pinch Gestures
// ====================
// Browser zoom:
// pinch direction="out" fingers=2 end="xdotool key ctrl+plus"
// pinch direction="in" fingers=2 end="xdotool key ctrl+minus"

// ====================
// Hold Gestures
// ====================
// Application launcher:
// hold fingers=4 action="rofi -show drun"

// Screenshot:
// hold fingers=3 action="flameshot gui"
"#
}

/// Generate or print default configuration file
fn generate_config(print_only: bool, force: bool) -> Result<()> {
    let config_content = get_default_config();

    if print_only {
        print!("{}", config_content);
        return Ok(());
    }

    // Get config directory
    let config_home = Config::get_config_home()?;

    let config_path = PathBuf::from(&config_home).join("gestures.kdl");

    // Check if file exists
    if config_path.exists() && !force {
        return Err(miette::miette!(
            "Config file already exists at: {}\nUse --force to overwrite, or --print to view the default config",
            config_path.display()
        ));
    }

    // Write config file
    let mut file = fs::File::create(&config_path).map_err(|e| {
        miette::miette!(
            "Failed to create config file {}: {}",
            config_path.display(),
            e
        )
    })?;

    file.write_all(config_content.as_bytes())
        .map_err(|e| miette::miette!("Failed to write config file: {}", e))?;

    println!("✓ Configuration file created at: {}", config_path.display());
    println!("\nEdit the file to customize your gestures:");
    println!("  vim {}", config_path.display());
    println!("\nAfter editing, reload the config:");
    println!("  gestures reload");
    println!("\nView full documentation:");
    println!("  https://github.com/ferstar/gestures/blob/dev/config.md");

    Ok(())
}

fn main() -> Result<()> {
    let app = App::parse();

    // Setup signal handlers for graceful shutdown
    signal_hook::flag::register(signal_hook::consts::SIGTERM, SHUTDOWN.clone())
        .map_err(|e| miette::miette!("Failed to register SIGTERM handler: {}", e))?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, SHUTDOWN.clone())
        .map_err(|e| miette::miette!("Failed to register SIGINT handler: {}", e))?;

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

    match app.command {
        c @ Commands::Reload => {
            ipc_client::handle_command(c)?;
        }
        Commands::Start => {
            let config_path = app.conf.clone();
            let c = Config::read_from_optional_path(config_path.as_deref()).unwrap_or_else(|e| {
                log::error!(
                    "Could not read configuration file, using empty config: {}",
                    e
                );
                Config::default()
            });
            log::debug!("{:#?}", c);

            let is_wayland = if app.wayland {
                log::info!("Forced Wayland mode via command line");
                true
            } else if app.x11 {
                log::info!("Forced X11 mode via command line");
                false
            } else {
                let detected = detect_wayland();
                log::info!(
                    "Auto-detected display server: {}",
                    if detected { "Wayland" } else { "X11" }
                );
                detected
            };
            run_eh(Arc::new(RwLock::new(c)), config_path, is_wayland)?;
        }
        Commands::InstallAutostart { print } => {
            install_autostart(print)?;
        }
        Commands::GenerateConfig { print, force } => {
            generate_config(print, force)?;
        }
    }

    Ok(())
}

fn run_eh(
    config: Arc<RwLock<Config>>,
    config_path: Option<PathBuf>,
    is_wayland: bool,
) -> Result<()> {
    let eh_thread = spawn_event_handler(config.clone(), is_wayland);
    if let Err(e) = ipc::create_socket(config, config_path) {
        SHUTDOWN.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = eh_thread.join();
        return Err(e);
    }

    let thread_result = eh_thread
        .join()
        .map_err(|_| miette::miette!("Event handler thread panicked"))?;
    thread_result?;

    Ok(())
}

fn spawn_event_handler(config: Arc<RwLock<Config>>, is_wayland: bool) -> JoinHandle<Result<()>> {
    thread::spawn(move || {
        log::debug!("Starting event handler in new thread");
        let mut eh = event_handler::EventHandler::new(config);
        let mut interface = input::Libinput::new_with_udev(event_handler::Interface);
        eh.init(&mut interface)?;
        eh.main_loop(&mut interface, &mut start_handler(!is_wayland))?;
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
    /// Force Wayland mode (default: auto-detect via WAYLAND_DISPLAY/XDG_SESSION_TYPE)
    #[arg(short = 'w', long)]
    wayland: bool,
    /// Force X11 mode (default: auto-detect)
    #[arg(short = 'x', long, conflicts_with = "wayland")]
    x11: bool,
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
    /// Install desktop autostart entry
    InstallAutostart {
        /// Print desktop entry to stdout instead of installing
        #[arg(short = 'p', long)]
        print: bool,
    },
    /// Generate default configuration file
    GenerateConfig {
        /// Print config to stdout instead of writing to file
        #[arg(short = 'p', long)]
        print: bool,
        /// Force overwrite existing config file
        #[arg(short = 'f', long)]
        force: bool,
    },
}
