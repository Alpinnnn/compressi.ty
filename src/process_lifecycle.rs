use std::{
    io::{self, Read},
    process::{Child, Command, Output},
    thread,
};

/// Starts a child process with the app's platform lifecycle guards applied.
pub fn spawn_child(command: &mut Command) -> io::Result<Child> {
    prepare_command(command);
    let child = command.spawn()?;
    register_child(&child);
    Ok(child)
}

/// Runs a child process and captures its output without Windows overlapped-pipe aborts.
pub fn output(command: &mut Command) -> io::Result<Output> {
    let mut child = spawn_child(command)?;
    drop(child.stdin.take());

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_reader = stdout.map(|pipe| thread::spawn(move || read_pipe_to_end(pipe)));
    let stderr_reader = stderr.map(|pipe| thread::spawn(move || read_pipe_to_end(pipe)));

    let status = child.wait()?;
    let stdout = join_reader(stdout_reader)?;
    let stderr = join_reader(stderr_reader)?;

    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// Reads a child-process pipe into text without using `read_to_end`/`read_to_string`.
pub fn read_pipe_to_string<R: Read>(reader: R) -> String {
    String::from_utf8_lossy(&read_pipe_to_end_lossy(reader)).into_owned()
}

fn join_reader(reader: Option<thread::JoinHandle<io::Result<Vec<u8>>>>) -> io::Result<Vec<u8>> {
    match reader {
        Some(reader) => reader
            .join()
            .map_err(|_| io::Error::other("process output reader panicked"))?,
        None => Ok(Vec::new()),
    }
}

fn read_pipe_to_end_lossy<R: Read>(reader: R) -> Vec<u8> {
    read_pipe_to_end(reader).unwrap_or_default()
}

fn read_pipe_to_end<R: Read>(mut reader: R) -> io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 8192];

    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => buffer.extend_from_slice(&chunk[..read]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }

    Ok(buffer)
}

fn prepare_command(command: &mut Command) {
    #[cfg(target_os = "linux")]
    linux::set_parent_death_signal(command);
    #[cfg(not(target_os = "linux"))]
    let _ = command;
}

#[cfg(not(target_os = "windows"))]
fn register_child(_child: &Child) {}

#[cfg(target_os = "linux")]
mod linux {
    use std::{io, os::unix::process::CommandExt, process::Command};

    const PR_SET_PDEATHSIG: i32 = 1;
    const SIGKILL: i32 = 9;

    unsafe extern "C" {
        fn prctl(option: i32, ...) -> i32;
        fn getppid() -> i32;
    }

    pub(super) fn set_parent_death_signal(command: &mut Command) {
        let parent_death_hook = || {
            if unsafe { prctl(PR_SET_PDEATHSIG, SIGKILL) } != 0 {
                return Err(io::Error::last_os_error());
            }

            if unsafe { getppid() } == 1 {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "parent exited before child process started",
                ));
            }

            Ok(())
        };

        unsafe {
            command.pre_exec(parent_death_hook);
        }
    }
}

#[cfg(target_os = "windows")]
fn register_child(child: &Child) {
    let _ = windows_job::assign_child(child.id());
}

#[cfg(target_os = "windows")]
mod windows_job {
    use std::{io, mem, ptr, sync::OnceLock};

    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            },
            Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE},
        },
    };

    struct AppJob(HANDLE);

    unsafe impl Send for AppJob {}
    unsafe impl Sync for AppJob {}

    static APP_JOB: OnceLock<Option<AppJob>> = OnceLock::new();

    pub(super) fn assign_child(process_id: u32) -> io::Result<()> {
        let Some(job) = app_job() else {
            return Ok(());
        };

        let process = unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, process_id) };
        if process == ptr::null_mut() {
            return Err(io::Error::last_os_error());
        }

        let assigned = unsafe { AssignProcessToJobObject(job, process) };
        unsafe {
            CloseHandle(process);
        }

        if assigned == 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }

    fn app_job() -> Option<HANDLE> {
        APP_JOB
            .get_or_init(|| create_app_job().ok())
            .as_ref()
            .map(|job| job.0)
    }

    fn create_app_job() -> io::Result<AppJob> {
        let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if job == ptr::null_mut() {
            return Err(io::Error::last_os_error());
        }

        let mut info = unsafe { mem::zeroed::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        let configured = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &mut info as *mut _ as *mut _,
                mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };

        if configured == 0 {
            let error = io::Error::last_os_error();
            unsafe {
                CloseHandle(job);
            }
            return Err(error);
        }

        Ok(AppJob(job))
    }
}

#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};

    #[test]
    fn captures_process_output_with_chunked_pipe_reads() {
        let mut command = output_probe_command();
        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        let output = super::output(&mut command).expect("process output should be captured");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(output.status.success());
        assert!(stdout.contains("stdout-199"));
        assert!(stderr.contains("stderr-199"));
    }

    #[cfg(target_os = "windows")]
    fn output_probe_command() -> Command {
        let mut command = Command::new("powershell.exe");
        command.args([
            "-NoProfile",
            "-Command",
            "1..200 | ForEach-Object { \"stdout-$_\" }; 1..200 | ForEach-Object { [Console]::Error.WriteLine(\"stderr-$_\") }",
        ]);
        command
    }

    #[cfg(not(target_os = "windows"))]
    fn output_probe_command() -> Command {
        let mut command = Command::new("sh");
        command.args([
            "-c",
            "i=0; while [ $i -lt 200 ]; do echo stdout-$i; echo stderr-$i >&2; i=$((i+1)); done",
        ]);
        command
    }
}
