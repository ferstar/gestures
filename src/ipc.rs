use std::env;
use std::io::{BufRead, BufReader};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, RwLock};
use std::thread;

use crate::config::Config;

pub fn create_socket(config: Arc<RwLock<Config>>) {
    let socket_dir = env::var("XDG_RUNTIME_DIR").unwrap_or("/tmp".to_string());
    let listener = UnixListener::bind(format!("{socket_dir}/gestures.sock")).unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let config = config.clone();
                thread::spawn(|| handle_connection(stream, config));
            }
            Err(err) => {
                eprintln!("Got error while handling IPC connection: {err}");
                break;
            }
        }
    }
    std::fs::remove_file(format!("{socket_dir}/gestures.sock")).unwrap();
}

fn handle_connection(stream: UnixStream, config: Arc<RwLock<Config>>) {
    let stream = BufReader::new(stream);

    for line in stream.lines() {
        if line.unwrap().contains("reload") {
            let mut c = config.write().unwrap();
            *c = Config::read_default_config().unwrap_or_else(|_| {
                log::error!("Could not read configuration file, using empty config!");
                Config::default()
            });
        }
    }
}
