use std::{
    path::Path,
    process::{Command, Stdio},
};

pub(super) fn background_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

pub(super) fn run_capture(mut command: Command) -> Result<String, String> {
    let output = crate::process_lifecycle::output(&mut command)
        .map_err(|error| format!("Could not start process: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        let detail = stderr
            .lines()
            .chain(stdout.lines())
            .find(|line| !line.trim().is_empty())
            .unwrap_or("Process exited unexpectedly.");
        return Err(detail.to_owned());
    }

    Ok(if stdout.trim().is_empty() {
        stderr.into_owned()
    } else {
        stdout.into_owned()
    })
}

pub(super) fn run_status(command: Command, label: &str) -> Result<(), String> {
    let output = run_capture(command).map_err(|error| format!("{label} failed: {error}"))?;
    if output.trim().is_empty() {
        return Ok(());
    }
    Ok(())
}
