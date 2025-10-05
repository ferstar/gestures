use miette::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use std::process::Command;

static REGEX_DELTA_X: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$delta_x").unwrap());
static REGEX_DELTA_Y: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$delta_y").unwrap());
static REGEX_SCALE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$scale").unwrap());
static REGEX_DELTA_ANGLE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$delta_angle").unwrap());

pub fn exec_command_from_string(args: &str, dx: f64, dy: f64, da: f64, scale: f64) -> Result<()> {
    if !args.is_empty() {
        let args = args.to_string();
        std::thread::spawn(move || {
            let args = REGEX_DELTA_Y.replace_all(&args, format!("{:.2}", dy));
            let args = REGEX_DELTA_X.replace_all(&args, format!("{:.2}", dx));
            let args = REGEX_SCALE.replace_all(&args, format!("{:.2}", scale));
            let args = REGEX_DELTA_ANGLE.replace_all(&args, format!("{:.2}", da));
            log::debug!("{:?}", &args);
            let _ = Command::new("sh")
                .arg("-c")
                .arg(&*args)
                .spawn()
                .and_then(|mut child| child.wait());
        });
    }
    Ok(())
}
