use miette::Result;
use parking_lot::Mutex;
use parking_lot::RwLock;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use crate::config::Config;

const IPC_WORKERS: usize = 4;
const IPC_QUEUE_CAPACITY: usize = 128;

fn current_uid() -> Option<u32> {
    fs::metadata("/proc/self").ok().map(|m| m.uid())
}

fn resolve_socket_path() -> Result<PathBuf> {
    if let Ok(socket_dir) = env::var("XDG_RUNTIME_DIR") {
        return Ok(PathBuf::from(socket_dir).join("gestures.sock"));
    }

    let uid = current_uid()
        .ok_or_else(|| miette::miette!("Cannot determine current uid from /proc/self"))?;
    let fallback = PathBuf::from(format!("/run/user/{uid}"));
    if fallback.is_dir() {
        Ok(fallback.join("gestures.sock"))
    } else {
        Err(miette::miette!(
            "XDG_RUNTIME_DIR is unset and fallback runtime dir {} is unavailable",
            fallback.display()
        ))
    }
}

fn remove_stale_socket(socket_path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(socket_path) {
        Ok(metadata) => metadata,
        Err(e) => {
            return Err(miette::miette!(
                "Failed to inspect existing IPC path {}: {}",
                socket_path.display(),
                e
            ));
        }
    };

    if !metadata.file_type().is_socket() {
        return Err(miette::miette!(
            "Refusing to remove non-socket path at {}",
            socket_path.display()
        ));
    }

    if let Some(uid) = current_uid() {
        if metadata.uid() != uid {
            return Err(miette::miette!(
                "Refusing to remove socket {} not owned by current user",
                socket_path.display()
            ));
        }
    }

    if let Err(e) = fs::remove_file(socket_path) {
        return Err(miette::miette!(
            "Could not remove existing socket file {}: {}",
            socket_path.display(),
            e
        ));
    }

    Ok(())
}

pub fn create_socket(
    config: Arc<RwLock<Config>>,
    config_path: Option<std::path::PathBuf>,
) -> Result<()> {
    let socket_path = resolve_socket_path()?;

    if socket_path.exists() {
        remove_stale_socket(&socket_path)?;
    }

    let listener = match UnixListener::bind(&socket_path) {
        Ok(listener) => listener,
        Err(e) => {
            return Err(miette::miette!(
                "Failed to bind IPC socket {}: {}",
                socket_path.display(),
                e
            ));
        }
    };

    // Set non-blocking mode
    if let Err(e) = listener.set_nonblocking(true) {
        let _ = fs::remove_file(&socket_path);
        return Err(miette::miette!("Cannot set non-blocking IPC socket: {}", e));
    }

    // Cleanup socket on shutdown
    let socket_path_clone = socket_path.clone();
    let cleanup = move || {
        let _ = fs::remove_file(&socket_path_clone);
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
                return Err(miette::miette!("IPC listener accept failed: {}", e));
            }
        }
    }

    Ok(())
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
