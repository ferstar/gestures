use std::env;
use std::io::Write;
use std::os::unix::net::UnixStream;

use miette::Result;

use crate::Commands;

pub fn handle_command(cmd: Commands) -> Result<()> {
    let socket_dir = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let socket_path = format!("{socket_dir}/gestures.sock");

    let mut stream = UnixStream::connect(&socket_path).map_err(|e| {
        miette::miette!(
            "Failed to connect to IPC socket {}: {}. Is gestures running?",
            socket_path,
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
