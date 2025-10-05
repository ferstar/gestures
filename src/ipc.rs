use parking_lot::RwLock;
use std::env;
use std::io::{BufRead, BufReader};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

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

    // Set non-blocking mode
    listener
        .set_nonblocking(true)
        .expect("Cannot set non-blocking");

    // Cleanup socket on shutdown
    let socket_path_clone = socket_path.clone();
    let cleanup = move || {
        let _ = std::fs::remove_file(&socket_path_clone);
    };

    // Register cleanup handler
    let socket_path_for_handler = socket_path.clone();
    ctrlc::set_handler(move || {
        let _ = std::fs::remove_file(&socket_path_for_handler);
        std::process::exit(0);
    })
    .unwrap();

    loop {
        // Check shutdown flag
        if crate::SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed) {
            log::info!("IPC listener shutting down");
            cleanup();
            break;
        }

        match listener.accept() {
            Ok((stream, _)) => {
                let config = config.clone();
                thread::spawn(|| handle_connection(stream, config));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No incoming connection, sleep briefly and check again
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                eprintln!("Got error while handling IPC connection: {e}");
                break;
            }
        }
    }
}

fn handle_connection(stream: UnixStream, config: Arc<RwLock<Config>>) {
    let stream = BufReader::new(stream);

    for line in stream.lines() {
        if line.unwrap().contains("reload") {
            let mut c = config.write();
            *c = Config::read_default_config().unwrap_or_else(|_| {
                log::error!("Could not read configuration file, using empty config!");
                Config::default()
            });
        }
    }
}
