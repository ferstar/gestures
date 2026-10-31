use miette::Result;
use once_cell::sync::Lazy;
use std::process::Command;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::thread;
use threadpool::ThreadPool;

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
            log::debug!("{:?}", args);
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
                    log::error!("Failed to execute command '{}': {}", args, e);
                }
            }
        });
    }
}

fn render_command(args: &str, dx: f64, dy: f64, da: f64, scale: f64) -> Option<String> {
    if args.is_empty() {
        return None;
    }

    Some(
        args.replace("$delta_y", &format!("{:.2}", dy))
            .replace("$delta_x", &format!("{:.2}", dx))
            .replace("$scale", &format!("{:.2}", scale))
            .replace("$delta_angle", &format!("{:.2}", da)),
    )
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

#[cfg(test)]
mod tests {
    use super::render_command;

    #[test]
    fn renders_all_variables_and_repeated_occurrences_with_two_decimals() {
        assert_eq!(
            render_command(
                "move $delta_x $delta_y; scale $scale; rotate $delta_angle; echo $delta_x $other",
                1.234,
                -2.346,
                -90.0,
                1.5,
            ),
            Some("move 1.23 -2.35; scale 1.50; rotate -90.00; echo 1.23 $other".into())
        );
    }

    #[test]
    fn handles_empty_and_literal_commands() {
        assert_eq!(render_command("", 0.0, 0.0, 0.0, 0.0), None);
        assert_eq!(
            render_command("echo '$HOME'  ", 0.0, 0.0, 0.0, 0.0),
            Some("echo '$HOME'  ".into())
        );
        assert_eq!(
            render_command("$delta_x $delta_y $delta_angle $scale", 0.0, 0.0, 0.0, 0.0),
            Some("0.00 0.00 0.00 0.00".into())
        );
    }
}
