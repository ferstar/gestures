use miette::Result;
use regex::Regex;
use std::process::Command;

pub fn exec_command_from_string(args: &str, dx: f64, dy: f64, scale: f64) -> Result<()> {
    if args.is_empty() {
        return Ok(());
    }

    let args = args.to_string();
    std::thread::spawn(move || {
        let rx = Regex::new(r"[^\\]\$delta_x").unwrap();
        let ry = Regex::new(r"[^\\]\$delta_y").unwrap();
        let rs = Regex::new(r"[^\\]\$scale").unwrap();
        let args = ry.replace_all(&args, format!(" {dy} "));
        let args = rx.replace_all(&args, format!(" {dx} "));
        let args = rs.replace_all(&args, format!(" {scale} "));
        log::debug!("{:?}", &args);
        Command::new("sh")
            .arg("-c")
            .arg(&*args)
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
    });

    Ok(())
}
