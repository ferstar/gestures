use anyhow::Result;
use regex::Regex;
use std::process::Command;

pub fn exec_command_from_string(args: &str, dx: f64, dy: f64, scale: f64) -> Result<()> {
    let rx = Regex::new(r"[^\\]\$delta_x")?;
    let ry = Regex::new(r"[^\\]\$delta_y")?;
    let rs = Regex::new(r"[^\\]\$scale")?;
    let args = ry.replace_all(args, format!(" {} ", dy));
    let args = rx.replace_all(&args, format!(" {} ", dx));
    let args = rs.replace_all(&args, format!(" {} ", scale));
    log::debug!("{:?}", &args);
    Command::new("sh").arg("-c").arg(&*args).spawn()?;
    Ok(())
}
