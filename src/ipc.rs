use parking_lot::Mutex;
use parking_lot::RwLock;
use std::env;
use std::io::{BufRead, BufReader};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use crate::config::Config;

const IPC_WORKERS: usize = 4;
const IPC_QUEUE_CAPACITY: usize = 128;

pub fn create_socket(config: Arc<RwLock<Config>>, config_path: Option<std::path::PathBuf>) {
    let socket_dir = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let socket_path = format!("{}/gestures.sock", socket_dir);

    if Path::new(&socket_path).exists() {
        std::fs::remove_file(&socket_path)
            .map_err(|e| {
                miette::miette!(
                    "Could not remove existing socket file {}: {}",
                    socket_path,
                    e
                )
            })
            .ok();
    }

    let listener = match UnixListener::bind(&socket_path) {
        Ok(listener) => listener,
        Err(e) => {
            log::error!("Failed to bind IPC socket {}: {}", socket_path, e);
            return;
        }
    };

    // Set non-blocking mode
    if let Err(e) = listener.set_nonblocking(true) {
        log::error!("Cannot set non-blocking IPC socket: {}", e);
        let _ = std::fs::remove_file(&socket_path);
        return;
    }

    // Cleanup socket on shutdown
    let socket_path_clone = socket_path.clone();
    let cleanup = move || {
        let _ = std::fs::remove_file(&socket_path_clone);
    };

    let (tx, rx) = mpsc::sync_channel::<UnixStream>(IPC_QUEUE_CAPACITY);
    let rx = Arc::new(Mutex::new(rx));

    for worker_id in 0..IPC_WORKERS {
        let config = config.clone();
        let config_path = config_path.clone();
        let rx = rx.clone();

        thread::spawn(move || loop {
            let stream = {
                let receiver = rx.lock();
                match receiver.recv() {
                    Ok(stream) => stream,
                    Err(_) => break,
                }
            };

            handle_connection(stream, config.clone(), config_path.clone());
            log::trace!("IPC worker {} handled one connection", worker_id);
        });
    }

    loop {
        // Check shutdown flag
        if crate::SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed) {
            log::info!("IPC listener shutting down");
            cleanup();
            break;
        }

        match listener.accept() {
            Ok((stream, _)) => {
                if let Err(e) = tx.try_send(stream) {
                    log::warn!("IPC queue is full or closed, dropping connection: {}", e);
                }
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

fn handle_connection(
    stream: UnixStream,
    config: Arc<RwLock<Config>>,
    config_path: Option<std::path::PathBuf>,
) {
    let stream = BufReader::new(stream);

    for line in stream.lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => {
                log::error!("Failed to read IPC command: {}", e);
                break;
            }
        };

        if line.contains("reload") {
            let mut c = config.write();
            *c = Config::read_from_optional_path(config_path.as_deref()).unwrap_or_else(|e| {
                log::error!(
                    "Could not read configuration file, using empty config: {}",
                    e
                );
                Config::default()
            });
        }
    }
}
