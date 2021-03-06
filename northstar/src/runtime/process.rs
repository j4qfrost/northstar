// Copyright (c) 2020 ESRLabs
//
//   Licensed under the Apache License, Version 2.0 (the "License");
//   you may not use this file except in compliance with the License.
//   You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
//   Unless required by applicable law or agreed to in writing, software
//   distributed under the License is distributed on an "AS IS" BASIS,
//   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//   See the License for the specific language governing permissions and
//   limitations under the License.

use super::{Event, EventTx};
use log::debug;
use nix::{
    sys::{signal, wait},
    unistd,
};
use std::fmt::Debug;
use thiserror::Error;
use tokio::{sync::mpsc, task};
use wait::WaitStatus;

pub(crate) const ENV_NAME: &str = "NAME";
pub(crate) const ENV_VERSION: &str = "VERSION";

pub type ExitCode = i32;
pub type Pid = u32;

#[derive(Clone, Debug)]
pub enum ExitStatus {
    /// Process exited with exit code
    Exit(ExitCode),
    /// Process was terminated by a signal
    Signaled(signal::Signal),
}

pub(crate) type ExitHandleWait = mpsc::Receiver<ExitStatus>;
type ExitHandleSignal = mpsc::Sender<ExitStatus>;

pub fn exit_handle() -> (ExitHandleSignal, ExitHandleWait) {
    mpsc::channel(1)
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to start process: {0}")]
    Start(String),
    #[error("Failed to stop process")]
    Stop,
    #[error("Wrong container type: {0}")]
    WrongContainerType(String),
    #[error("Minijail error: {0}")]
    Minijail(#[from] ::minijail::Error),
    #[error("IO error: {0}: {1:?}")]
    Io(String, std::io::Error),
    #[error("OS error: {0}: {1:?}")]
    Os(String, nix::Error),
}

/// Spawn a task that waits for the process to exit. Once the process is exited send the return code
// (if any) to the exit_tx handle passed
pub(crate) async fn waitpid(
    name: &str,
    pid: u32,
    exit_handle: ExitHandleSignal,
    event_handle: EventTx,
) {
    let name = name.to_string();
    task::spawn_blocking(move || {
        let pid = unistd::Pid::from_raw(pid as i32);
        let status = loop {
            let result = wait::waitpid(Some(pid), None);
            debug!("Result of wait_pid is {:?}", result);

            match result {
                // The process exited normally (as with exit() or returning from main) with the given exit code.
                // This case matches the C macro WIFEXITED(status); the second field is WEXITSTATUS(status).
                Ok(WaitStatus::Exited(_pid, code)) => break ExitStatus::Exit(code),

                // The process was killed by the given signal.
                // The third field indicates whether the signal generated a core dump. This case matches the C macro WIFSIGNALED(status); the last two fields correspond to WTERMSIG(status) and WCOREDUMP(status).
                Ok(WaitStatus::Signaled(_pid, signal, _dump)) => {
                    break ExitStatus::Signaled(signal);
                }

                // The process is alive, but was stopped by the given signal.
                // This is only reported if WaitPidFlag::WUNTRACED was passed. This case matches the C macro WIFSTOPPED(status); the second field is WSTOPSIG(status).
                Ok(WaitStatus::Stopped(_pid, _signal)) => continue,

                // The traced process was stopped by a PTRACE_EVENT_* event.
                // See nix::sys::ptrace and ptrace(2) for more information. All currently-defined events use SIGTRAP as the signal; the third field is the PTRACE_EVENT_* value of the event.
                #[cfg(any(target_os = "linux", target_os = "android"))]
                Ok(WaitStatus::PtraceEvent(_pid, _signal, _)) => continue,

                // The traced process was stopped by execution of a system call, and PTRACE_O_TRACESYSGOOD is in effect.
                // See ptrace(2) for more information.
                #[cfg(any(target_os = "linux", target_os = "android"))]
                Ok(WaitStatus::PtraceSyscall(_pid)) => continue,

                // The process was previously stopped but has resumed execution after receiving a SIGCONT signal.
                // This is only reported if WaitPidFlag::WCONTINUED was passed. This case matches the C macro WIFCONTINUED(status).
                Ok(WaitStatus::Continued(_pid)) => continue,

                // There are currently no state changes to report in any awaited child process.
                // This is only returned if WaitPidFlag::WNOHANG was used (otherwise wait() or waitpid() would block until there was something to report).
                Ok(WaitStatus::StillAlive) => continue,
                // Retry the waitpid call if waitpid fails with EINTR
                Err(e) if e == nix::Error::Sys(nix::errno::Errno::EINTR) => continue,
                Err(e) => panic!("Failed to waitpid on {}: {}", pid, e),
            }
        };

        // Send notification to exit handle
        exit_handle.blocking_send(status.clone()).ok();

        // Send notification to main loop
        event_handle
            .blocking_send(Event::Exit(name.to_string(), status))
            .expect("Internal channel error on main event handle");
    });
}
