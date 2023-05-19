use std::env;
use std::io::{BufRead, BufReader};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, RwLock};
use std::thread;

use crate::config::Config;

struct IpcListener(UnixListener);

impl Drop for IpcListener {
    fn drop(&mut self) {
        let addr = self.0.local_addr().expect("Couldn't get socket address");
        let path = addr.as_pathname().expect("Socket address is not a path");
        std::fs::remove_file(path).expect("Could not remove socket");
    }
}

pub fn create_socket(config: Arc<RwLock<Config>>) {
    let socket_dir = env::var("XDG_RUNTIME_DIR").unwrap_or("/tmp".to_string());
    let socket_path = format!("{}/gestures.sock", socket_dir);
    if std::path::Path::new(&socket_path).exists() {
        std::fs::remove_file(&socket_path).expect("Could not remove existing socket file");
    }
    let listener = UnixListener::bind(&socket_path).unwrap();

    {
        // let listener = listener.clone();
        ctrlc::set_handler(move || {
            std::fs::remove_file(&socket_path).unwrap();
            std::process::exit(1);
        })
        .unwrap();
    }

    // for stream in listener.read().unwrap().0.incoming() {
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
