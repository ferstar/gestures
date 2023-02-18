use miette::{IntoDiagnostic, Result};
use regex::Regex;
use std::process::Command;

pub fn exec_command_from_string(args: &str, dx: f64, dy: f64, scale: f64) -> Result<()> {
    if !&args.is_empty() {
        let rx = Regex::new(r"[^\\]\$delta_x").into_diagnostic()?;
        let ry = Regex::new(r"[^\\]\$delta_y").into_diagnostic()?;
        let rs = Regex::new(r"[^\\]\$scale").into_diagnostic()?;
        let args = ry.replace_all(args, format!(" {dy} "));
        let args = rx.replace_all(&args, format!(" {dx} "));
        let args = rs.replace_all(&args, format!(" {scale} "));
        log::debug!("{:?}", &args);
        Command::new("sh").arg("-c").arg(&*args).spawn().into_diagnostic()?;
    }
    Ok(())
}
