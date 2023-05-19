use std::env;
use std::io::Write;
use std::os::unix::net::UnixStream;

use crate::Commands;

pub fn handle_command(cmd: Commands) {
    let socket_dir = env::var("XDG_RUNTIME_DIR").unwrap_or("/tmp".to_string());
    let mut stream = match UnixStream::connect(format!("{socket_dir}/gestures.sock")) {
        Ok(s) => s,
        Err(e) => panic!("Got this while trying to connect to ipc: {e} \nPerhaps the main program is not running"),
    };
    #[allow(clippy::single_match)]
    match cmd {
        Commands::Reload => {
            stream.write_all(b"reload").unwrap();
        }
        _ => (),
    }
}
