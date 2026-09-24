//! One request run in a worker process with a timeout; a crash, a hang or garbage comes back as a [`Failure`].
use crate::embedded::Embedded;
use crate::protocol::{MARKER, Request, Response};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The environment variable that names the worker program, over the one beside the executable.
pub const WORKER_ENV: &str = "RINGDESIGN_OCCT_WORKER";
/// The worker program's name beside the executable.
pub const WORKER_NAME: &str = "occt-worker";
/// The argument a host binary that also serves as its own worker answers to.
pub const WORKER_FLAG: &str = "--occt-worker";
/// How much of a worker's stderr a failure carries, bytes.
const STDERR_TAIL: usize = 2048;

/// Why a request came back without a response.
#[derive(Debug)]
pub enum Failure {
    /// No worker where one was looked for.
    Missing(PathBuf),
    /// The worker would not start.
    Spawn(String),
    /// It ran past the timeout and was killed.
    Timeout(Duration),
    /// Its caller stopped it, and it was killed.
    Cancelled,
    /// It died without answering: how it ended and the tail of its stderr.
    Crashed { status: String, stderr: String },
    /// It answered something that is not a response.
    Garbled(String),
    /// The worker the executable carries would not come out of it.
    Unpack(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(p) => write!(f, "OpenCascade's worker is not at {}", p.display()),
            Self::Spawn(e) => write!(f, "OpenCascade's worker would not start: {e}"),
            Self::Timeout(t) => write!(f, "OpenCascade ran past {:.1} s and was stopped", t.as_secs_f64()),
            Self::Cancelled => write!(f, "OpenCascade was stopped before it answered"),
            Self::Crashed { status, stderr } if stderr.is_empty() => write!(f, "OpenCascade's worker died ({status})"),
            Self::Crashed { status, stderr } => write!(f, "OpenCascade's worker died ({status}): {stderr}"),
            Self::Garbled(e) => write!(f, "OpenCascade's worker answered nonsense: {e}"),
            Self::Unpack(e) => write!(f, "OpenCascade's worker would not unpack: {e}"),
        }
    }
}

impl std::error::Error for Failure {}

/// Where a worker would come from, found without unpacking or starting anything.
#[derive(Clone, Debug, PartialEq)]
pub enum Found {
    /// Named by [`WORKER_ENV`].
    Named(PathBuf),
    /// [`WORKER_NAME`] beside the running executable.
    Beside(PathBuf),
    /// Carried inside the executable; `unpacked` once a copy of it sits in the data folder.
    Embedded { unpacked: bool },
}

/// [`WORKER_NAME`] beside the running executable, whether or not it is there.
fn beside_executable() -> Result<PathBuf, Failure> {
    let exe = std::env::current_exe().map_err(|e| Failure::Spawn(e.to_string()))?;
    Ok(exe.with_file_name(format!("{WORKER_NAME}{}", std::env::consts::EXE_SUFFIX)))
}

/// The worker [`Worker::find`] would run: the one [`WORKER_ENV`] names, else one beside the executable, else `embedded` under `root`.
pub fn probe(embedded: &Embedded, root: &Path) -> Result<Found, Failure> {
    if let Some(named) = std::env::var_os(WORKER_ENV) {
        let named = PathBuf::from(named);
        return if named.is_file() { Ok(Found::Named(named)) } else { Err(Failure::Missing(named)) };
    }
    let beside = beside_executable()?;
    if beside.is_file() {
        return Ok(Found::Beside(beside));
    }
    if embedded.is_present() {
        return Ok(Found::Embedded { unpacked: embedded.is_unpacked(root) });
    }
    Err(Failure::Missing(beside))
}

/// A worker program and the arguments it is started with.
#[derive(Clone, Debug, PartialEq)]
pub struct Worker {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl Worker {
    pub fn at(program: impl Into<PathBuf>) -> Self {
        Self { program: program.into(), args: Vec::new() }
    }

    /// [`WORKER_ENV`] when it is set, else [`WORKER_NAME`] beside the running executable.
    pub fn locate() -> Result<Self, Failure> {
        let program = match std::env::var_os(WORKER_ENV) {
            Some(p) => PathBuf::from(p),
            None => beside_executable()?,
        };
        if !program.is_file() {
            return Err(Failure::Missing(program));
        }
        Ok(Self::at(program))
    }

    /// [`Worker::locate`]'s worker when there is one, else `embedded` unpacked under `root` on its first use; a name in [`WORKER_ENV`] is never passed over.
    pub fn find(embedded: &Embedded, root: &Path) -> Result<Self, Failure> {
        match probe(embedded, root)? {
            Found::Named(program) | Found::Beside(program) => Ok(Self::at(program)),
            Found::Embedded { .. } => embedded.unpack(root).map(|u| Self::at(u.path)),
        }
    }

    /// The running executable answering [`WORKER_FLAG`], for a host that dispatches to `kernel::serve` itself.
    pub fn this_executable() -> Result<Self, Failure> {
        let program = std::env::current_exe().map_err(|e| Failure::Spawn(e.to_string()))?;
        Ok(Self { program, args: vec![WORKER_FLAG.to_string()] })
    }

    /// `request` run in a fresh worker, killed once `timeout` passes.
    pub fn run(&self, request: &Request, timeout: Duration) -> Result<Response, Failure> {
        self.run_cancellable(request, timeout, &AtomicBool::new(false))
    }

    /// [`Worker::run`], its worker killed as soon as `cancel` is set.
    pub fn run_cancellable(&self, request: &Request, timeout: Duration, cancel: &AtomicBool) -> Result<Response, Failure> {
        let input = serde_json::to_vec(request).map_err(|e| Failure::Spawn(e.to_string()))?;
        let mut command = Command::new(&self.program);
        command.args(&self.args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: a console worker started by a windowed app opens no console of its own.
            command.creation_flags(0x0800_0000);
        }
        let mut child = command.spawn().map_err(|e| Failure::Spawn(format!("{}: {e}", self.program.display())))?;
        let mut stdin = child.stdin.take().ok_or_else(|| Failure::Spawn("no stdin".into()))?;
        let mut stdout = child.stdout.take().ok_or_else(|| Failure::Spawn("no stdout".into()))?;
        let mut stderr = child.stderr.take().ok_or_else(|| Failure::Spawn("no stderr".into()))?;
        // Stdin, stdout and stderr each on a thread of its own.
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(&input);
        });
        let reader = std::thread::spawn(move || {
            let mut out = Vec::new();
            let _ = stdout.read_to_end(&mut out);
            out
        });
        let errors = std::thread::spawn(move || {
            let mut err = Vec::new();
            let _ = stderr.read_to_end(&mut err);
            err
        });
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if cancel.load(Ordering::Relaxed) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    drop((writer, reader, errors));
                    return Err(Failure::Cancelled);
                }
                Ok(None) if started.elapsed() >= timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    // The pipe threads are left to end when their pipes close.
                    drop((writer, reader, errors));
                    return Err(Failure::Timeout(timeout));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(2)),
                Err(e) => return Err(Failure::Spawn(e.to_string())),
            }
        };
        let _ = writer.join();
        let out = reader.join().unwrap_or_default();
        let err = errors.join().unwrap_or_default();
        let tail = {
            let text = String::from_utf8_lossy(&err);
            let text = text.trim();
            let start = text.char_indices().rev().nth(STDERR_TAIL).map_or(0, |(i, _)| i);
            text[start..].to_string()
        };
        let text = String::from_utf8_lossy(&out);
        let Some(at) = text.rfind(MARKER) else {
            return Err(if status.success() { Failure::Garbled("no response".into()) } else { Failure::Crashed { status: status.to_string(), stderr: tail } });
        };
        match serde_json::from_str::<Response>(text[at + MARKER.len()..].trim()) {
            Ok(response) => Ok(response),
            Err(_) if !status.success() => Err(Failure::Crashed { status: status.to_string(), stderr: tail }),
            Err(e) => Err(Failure::Garbled(e.to_string())),
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// A shell script standing in for the worker.
    fn script(body: &str) -> Worker {
        Worker { program: "/bin/sh".into(), args: vec!["-c".into(), body.into()] }
    }

    #[test]
    fn a_hang_a_crash_and_nonsense_come_back_as_failures_not_as_the_callers() {
        let hang = script("sleep 30").run(&Request::Ping, Duration::from_millis(300));
        assert!(matches!(hang, Err(Failure::Timeout(_))), "{hang:?}");
        let crash = script("cat > /dev/null; echo 'Standard_Failure: boom' >&2; kill -SEGV $$").run(&Request::Ping, Duration::from_secs(5));
        let Err(Failure::Crashed { status, stderr }) = crash else { panic!("{crash:?}") };
        assert!(status.contains("SIGSEGV") && stderr == "Standard_Failure: boom", "{status} {stderr}");
        let nonsense = script(&format!("cat > /dev/null; echo '{MARKER}'; echo '{{\"outcome\":\"sideways\"}}'")).run(&Request::Ping, Duration::from_secs(5));
        assert!(matches!(nonsense, Err(Failure::Garbled(_))), "{nonsense:?}");
        let missing = Worker::at("/nowhere/occt-worker").run(&Request::Ping, Duration::from_secs(5));
        assert!(matches!(missing, Err(Failure::Spawn(_))), "{missing:?}");
    }

    #[test]
    fn a_cancelled_run_kills_its_worker_at_once() {
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            flag.store(true, Ordering::Relaxed);
        });
        let started = Instant::now();
        let stopped = script("sleep 30").run_cancellable(&Request::Ping, Duration::from_secs(60), &cancel);
        assert!(matches!(stopped, Err(Failure::Cancelled)), "{stopped:?}");
        assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
    }

    #[test]
    fn what_opencascade_prints_before_the_answer_is_skipped() {
        let answer = serde_json::to_string(&Response::Refused { message: "no".into() }).unwrap();
        let chatty = script(&format!("cat > /dev/null; echo '*** Warning: STEP header'; echo '{MARKER}'; echo '{answer}'"));
        assert_eq!(chatty.run(&Request::Ping, Duration::from_secs(5)).unwrap(), Response::Refused { message: "no".into() });
    }
}
