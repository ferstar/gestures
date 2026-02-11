use miette::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use std::process::Command;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::thread;
use threadpool::ThreadPool;

static REGEX_DELTA_X: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$delta_x").unwrap());
static REGEX_DELTA_Y: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$delta_y").unwrap());
static REGEX_SCALE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$scale").unwrap());
static REGEX_DELTA_ANGLE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$delta_angle").unwrap());

// Thread pool with 4 workers to handle command execution
static THREAD_POOL: Lazy<ThreadPool> = Lazy::new(|| ThreadPool::new(4));
const COMMAND_QUEUE_CAPACITY: usize = 256;

static COMMAND_SENDER: Lazy<SyncSender<String>> = Lazy::new(|| {
    let (tx, rx) = sync_channel(COMMAND_QUEUE_CAPACITY);
    thread::spawn(move || command_dispatch_loop(rx));
    tx
});

fn command_dispatch_loop(rx: Receiver<String>) {
    while let Ok(args) = rx.recv() {
        THREAD_POOL.execute(move || {
            log::debug!("{:?}", &args);
            match Command::new("sh").arg("-c").arg(&args).status() {
                Ok(status) => {
                    if !status.success() {
                        log::warn!(
                            "Command exited with non-zero status '{}': {:?}",
                            args,
                            status
                        );
                    }
                }
                Err(e) => {
                    log::error!("Failed to execute command '{}': {}", &args, e);
                }
            }
        });
    }
}

fn render_command(args: &str, dx: f64, dy: f64, da: f64, scale: f64) -> Option<String> {
    if args.is_empty() {
        return None;
    }

    let args = REGEX_DELTA_Y.replace_all(args, format!("{:.2}", dy));
    let args = REGEX_DELTA_X.replace_all(&args, format!("{:.2}", dx));
    let args = REGEX_SCALE.replace_all(&args, format!("{:.2}", scale));
    let args = REGEX_DELTA_ANGLE.replace_all(&args, format!("{:.2}", da));
    Some(args.to_string())
}

fn enqueue_command(args: String, drop_when_full: bool) -> Result<()> {
    match COMMAND_SENDER.try_send(args) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(cmd)) if drop_when_full => {
            log::debug!("Command queue is full, dropping update command: {}", cmd);
            Ok(())
        }
        Err(TrySendError::Full(cmd)) => COMMAND_SENDER
            .send(cmd)
            .map_err(|e| miette::miette!("Failed to enqueue command: {}", e)),
        Err(TrySendError::Disconnected(_)) => {
            Err(miette::miette!("Command queue dispatcher disconnected"))
        }
    }
}

pub fn exec_command_from_string(args: &str, dx: f64, dy: f64, da: f64, scale: f64) -> Result<()> {
    if let Some(args) = render_command(args, dx, dy, da, scale) {
        enqueue_command(args, false)?;
    }
    Ok(())
}

pub fn exec_update_command_from_string(
    args: &str,
    dx: f64,
    dy: f64,
    da: f64,
    scale: f64,
) -> Result<()> {
    if let Some(args) = render_command(args, dx, dy, da, scale) {
        enqueue_command(args, true)?;
    }
    Ok(())
}
