use std::env;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use miette::Result;

use crate::Commands;

fn current_uid() -> Option<u32> {
    std::fs::metadata("/proc/self").ok().map(|m| m.uid())
}

fn socket_path() -> Option<PathBuf> {
    if let Ok(socket_dir) = env::var("XDG_RUNTIME_DIR") {
        return Some(PathBuf::from(socket_dir).join("gestures.sock"));
    }

    let uid = current_uid()?;
    let fallback = PathBuf::from(format!("/run/user/{uid}"));
    if fallback.is_dir() {
        Some(fallback.join("gestures.sock"))
    } else {
        None
    }
}

pub fn handle_command(cmd: Commands) -> Result<()> {
    let socket_path = socket_path().ok_or_else(|| {
        miette::miette!("Could not determine IPC runtime directory (XDG_RUNTIME_DIR is unset)")
    })?;

    let mut stream = UnixStream::connect(&socket_path).map_err(|e| {
        miette::miette!(
            "Failed to connect to IPC socket {}: {}. Is gestures running?",
            socket_path.display(),
            e
        )
    })?;

    #[allow(clippy::single_match)]
    match cmd {
        Commands::Reload => {
            stream
                .write_all(b"reload")
                .map_err(|e| miette::miette!("Failed to write reload command: {}", e))?;
        }
        _ => (),
    }

    Ok(())
}
