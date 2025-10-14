mod config;
mod event_handler;
mod gestures;
mod ipc;
mod ipc_client;
mod mouse_handler;
mod utils;

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

/// Generate systemd user service file content
fn generate_service_file() -> Result<String> {
    let exe_path = env::current_exe()
        .map_err(|e| miette::miette!("Failed to get current executable path: {}", e))?;

    let exe_path_str = exe_path
        .to_str()
        .ok_or_else(|| miette::miette!("Executable path contains invalid UTF-8"))?;

    let display = env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());

    let service_content = format!(
        r#"[Unit]
Description=Touchpad Gestures (with 3-finger drag performance improvements)
Documentation=https://github.com/ferstar/gestures

[Service]
Environment=PATH=/usr/local/bin:/usr/local/sbin:/usr/bin:/bin
Environment=DISPLAY={}
Type=simple
ExecStart={} start
ExecReload={} reload
Restart=never

[Install]
WantedBy=default.target
"#,
        display, exe_path_str, exe_path_str
    );

    Ok(service_content)
}

/// Install or print systemd user service file
fn install_service(print_only: bool) -> Result<()> {
    let service_content = generate_service_file()?;

    if print_only {
        print!("{}", service_content);
        return Ok(());
    }

    // Get user's systemd directory
    let home =
        env::var("HOME").map_err(|_| miette::miette!("HOME environment variable not set"))?;

    let systemd_dir = PathBuf::from(home).join(".config/systemd/user");
    let service_path = systemd_dir.join("gestures.service");

    // Create directory if it doesn't exist
    fs::create_dir_all(&systemd_dir).map_err(|e| {
        miette::miette!(
            "Failed to create directory {}: {}",
            systemd_dir.display(),
            e
        )
    })?;

    // Write service file
    let mut file = fs::File::create(&service_path).map_err(|e| {
        miette::miette!(
            "Failed to create service file {}: {}",
            service_path.display(),
            e
        )
    })?;

    file.write_all(service_content.as_bytes())
        .map_err(|e| miette::miette!("Failed to write service file: {}", e))?;

    println!("✓ Service file installed to: {}", service_path.display());
    println!("\nTo enable and start the service, run:");
    println!("  systemctl --user enable --now gestures.service");
    println!("\nTo view service status:");
    println!("  systemctl --user status gestures.service");

    Ok(())
}

/// Generate default configuration content
fn get_default_config() -> &'static str {
    r#"// Gestures Configuration
// See https://github.com/ferstar/gestures for full documentation

// ====================
// 3-Finger Drag (macOS-like)
// ====================
// Works on both X11 and Wayland
// - X11: Uses libxdo API directly (minimal latency)
// - Wayland: Uses ydotool (ensure ydotoold daemon is running)
swipe direction="any" fingers=3 mouse-up-delay=500 acceleration=20

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
    let config_home = env::var("XDG_CONFIG_HOME")
        .unwrap_or_else(|_| format!("{}/.config", env::var("HOME").unwrap()));

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
        Commands::Start => {
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
            run_eh(Arc::new(RwLock::new(c)), is_wayland)?;
        }
        Commands::InstallService { print } => {
            install_service(print)?;
        }
        Commands::GenerateConfig { print, force } => {
            generate_config(print, force)?;
        }
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
    /// Install systemd user service
    InstallService {
        /// Print service file to stdout instead of installing
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
