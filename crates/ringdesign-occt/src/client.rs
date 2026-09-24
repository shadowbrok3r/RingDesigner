//! One request run in a worker process with a timeout; a crash, a hang or garbage comes back as a [`Failure`].
use crate::embedded::Embedded;
use crate::protocol::{MARKER, Request, Response};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The environment variable that names the worker program, over the one beside the executable; a relative path is read from the executable's folder.
pub const WORKER_ENV: &str = "RINGDESIGN_OCCT_WORKER";
/// The name a host's build reads its worker from, read at run time as well when [`WORKER_ENV`] is unset, a relative path from the executable's folder.
pub const WORKER_ENV_ALIAS: &str = "RINGDESIGNER_OCCT_WORKER";
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
    /// Named by [`WORKER_ENV`] or [`WORKER_ENV_ALIAS`].
    Named(PathBuf),
    /// [`WORKER_NAME`] beside the running executable.
    Beside(PathBuf),
    /// Carried inside the executable; `unpacked` once a copy of it sits in the data folder.
    Embedded { unpacked: bool },
}

/// `path` as it stands when absolute or when there is no `folder`, else read from `folder`.
fn anchored(path: PathBuf, folder: Option<&std::path::Path>) -> PathBuf {
    match folder {
        Some(folder) if path.is_relative() => folder.join(path),
        _ => path,
    }
}

/// [`WORKER_NAME`] beside the running executable, whether or not it is there.
fn beside_executable() -> Result<PathBuf, Failure> {
    let exe = std::env::current_exe().map_err(|e| Failure::Spawn(e.to_string()))?;
    Ok(exe.with_file_name(format!("{WORKER_NAME}{}", std::env::consts::EXE_SUFFIX)))
}

/// Where a host looks for its worker, in order: the path a variable names, [`WORKER_NAME`] beside the executable, the worker it carries.
#[derive(Clone, Debug)]
pub struct Locator {
    /// The variable that named a worker and the path it names; a named worker is never passed over.
    pub named: Option<(&'static str, PathBuf)>,
    /// [`WORKER_NAME`] beside the running executable, whether or not it is there; `None` where the executable cannot be found.
    pub beside: Option<PathBuf>,
    /// The worker the host carries.
    pub embedded: Embedded,
    /// The folder the carried worker unpacks under.
    pub root: PathBuf,
}

impl Locator {
    /// Read from the environment once: [`WORKER_ENV`], else [`WORKER_ENV_ALIAS`], then beside the executable, then `embedded` under `root`.
    pub fn from_env(embedded: Embedded, root: PathBuf) -> Self {
        Self::from_vars(embedded, root, |var| std::env::var_os(var))
    }

    /// [`Locator::from_env`] with the variables read through `var`; an empty value counts as unset, a relative path is read from the executable's folder.
    pub fn from_vars(embedded: Embedded, root: PathBuf, var: impl Fn(&str) -> Option<std::ffi::OsString>) -> Self {
        let beside = beside_executable().ok();
        let folder = beside.as_ref().and_then(|b| b.parent());
        let named = [WORKER_ENV, WORKER_ENV_ALIAS].into_iter().find_map(|name| var(name).filter(|v| !v.is_empty()).map(|v| (name, anchored(PathBuf::from(v), folder))));
        Self { named, beside, embedded, root }
    }

    /// Only the worker at `program`, as [`WORKER_ENV`] would name it.
    pub fn named(program: impl Into<PathBuf>) -> Self {
        Self { named: Some((WORKER_ENV, program.into())), ..Self::nowhere() }
    }

    /// No worker anywhere.
    pub fn nowhere() -> Self {
        Self { named: None, beside: None, embedded: Embedded::NONE, root: PathBuf::new() }
    }

    /// The worker [`Locator::find`] would run, found without unpacking or starting anything.
    pub fn probe(&self) -> Result<Found, Failure> {
        if let Some((_, named)) = &self.named {
            return if named.is_file() { Ok(Found::Named(named.clone())) } else { Err(Failure::Missing(named.clone())) };
        }
        if let Some(beside) = self.beside.as_ref().filter(|b| b.is_file()) {
            return Ok(Found::Beside(beside.clone()));
        }
        if self.embedded.is_present() {
            return Ok(Found::Embedded { unpacked: self.embedded.is_unpacked(&self.root) });
        }
        Err(Failure::Missing(self.beside.clone().unwrap_or_else(|| PathBuf::from(format!("{WORKER_NAME}{}", std::env::consts::EXE_SUFFIX)))))
    }

    /// The worker [`Locator::probe`] names, the carried one unpacked on its first use.
    pub fn find(&self) -> Result<Worker, Failure> {
        self.find_cancellable(&AtomicBool::new(false))
    }

    /// [`Locator::find`], a first unpack stopped as soon as `cancel` is set.
    pub fn find_cancellable(&self, cancel: &AtomicBool) -> Result<Worker, Failure> {
        match self.probe()? {
            Found::Named(program) | Found::Beside(program) => Ok(Worker::at(program)),
            Found::Embedded { .. } => self.embedded.unpack_cancellable(&self.root, cancel).map(|u| Worker::at(u.path)),
        }
    }
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
    fn a_relative_worker_path_is_read_from_the_executables_folder_wherever_the_app_starts() {
        let folder = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
        let named = |path: &str| Locator::from_vars(Embedded::NONE, PathBuf::new(), move |var| (var == WORKER_ENV_ALIAS).then(|| path.into())).named;
        assert_eq!(named("occt-embed/linux/occt-worker"), Some((WORKER_ENV_ALIAS, folder.join("occt-embed/linux/occt-worker"))));
        assert_eq!(named("../bin/occt-worker"), Some((WORKER_ENV_ALIAS, folder.join("../bin/occt-worker"))));
        assert_eq!(named("/opt/occt-worker"), Some((WORKER_ENV_ALIAS, PathBuf::from("/opt/occt-worker"))));
        // Found where the executable is, not where the process was started.
        let beside = Locator::from_vars(Embedded::NONE, PathBuf::new(), |_| None).beside.unwrap();
        let name = beside.file_name().unwrap().to_str().unwrap().to_string();
        let relative = Locator::from_vars(Embedded::NONE, PathBuf::new(), move |var| (var == WORKER_ENV).then(|| name.clone().into())).named.unwrap();
        assert_eq!(relative, (WORKER_ENV, beside));
        assert_eq!(anchored(PathBuf::from("w"), None), PathBuf::from("w"));
    }

    #[test]
    fn what_opencascade_prints_before_the_answer_is_skipped() {
        let answer = serde_json::to_string(&Response::Refused { message: "no".into() }).unwrap();
        let chatty = script(&format!("cat > /dev/null; echo '*** Warning: STEP header'; echo '{MARKER}'; echo '{answer}'"));
        assert_eq!(chatty.run(&Request::Ping, Duration::from_secs(5)).unwrap(), Response::Refused { message: "no".into() });
    }
}
