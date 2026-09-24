//! The OpenCascade worker this build carries, if any, where it unpacks, and whether the app has a worker at all.
use ringdesign_occt::client::{self, Failure, Found, WORKER_ENV, WORKER_NAME, Worker};
use ringdesign_occt::embedded::Embedded;
use std::path::PathBuf;

include!(concat!(env!("OUT_DIR"), "/occt_worker.rs"));

/// The folder an embedded worker unpacks under: the library's data root, or on Windows the local, unroamed app data.
pub fn root() -> PathBuf {
    let local = std::env::var_os("LOCALAPPDATA").filter(|_| cfg!(windows)).map(|d| PathBuf::from(d).join("RingDesigner"));
    let data = Some(ringdesign_core::library::data_root()).filter(|d| d.is_absolute());
    local.or(data).unwrap_or_else(|| std::env::temp_dir().join("ringdesigner"))
}

/// Where the worker would come from, found without unpacking or starting anything.
pub fn probe() -> Result<Found, Failure> {
    client::probe(&EMBEDDED, &root())
}

/// Why OpenCascade cannot be offered, or `None` when a worker is there.
pub fn missing() -> Option<String> {
    reason(&probe())
}

/// What a probe says to someone deciding whether to use OpenCascade.
pub fn reason(probe: &Result<Found, Failure>) -> Option<String> {
    let exe = format!("{WORKER_NAME}{}", std::env::consts::EXE_SUFFIX);
    match probe {
        Ok(_) => None,
        Err(Failure::Missing(p)) if std::env::var_os(WORKER_ENV).is_some() => Some(format!("{WORKER_ENV} names {}, which is not there", p.display())),
        Err(Failure::Missing(_)) => Some(format!("This build carries no OpenCascade worker: put {exe} beside RingDesigner, or name one in {WORKER_ENV}")),
        Err(e) => Some(e.to_string()),
    }
}

/// The worker to run, the embedded one unpacked on its first use; the wait is logged.
pub fn worker() -> Result<Worker, String> {
    let fresh = unpacks_first();
    let started = std::time::Instant::now();
    let worker = Worker::find(&EMBEDDED, &root()).map_err(|e| e.to_string())?;
    if fresh {
        log::info!("OpenCascade's worker unpacked to {} in {:.0} ms", worker.program.display(), started.elapsed().as_secs_f64() * 1e3);
    }
    Ok(worker)
}

/// Whether the next run first unpacks the worker this build carries.
pub fn unpacks_first() -> bool {
    matches!(probe(), Ok(Found::Embedded { unpacked: false }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_worker_says_where_one_would_go() {
        let beside = Err(Failure::Missing(PathBuf::from("/opt/ringdesigner/occt-worker")));
        let named = std::env::var_os(WORKER_ENV).is_some();
        let said = reason(&beside).unwrap();
        if named {
            assert_eq!(said, format!("{WORKER_ENV} names /opt/ringdesigner/occt-worker, which is not there"));
        } else {
            assert_eq!(said, format!("This build carries no OpenCascade worker: put occt-worker{} beside RingDesigner, or name one in {WORKER_ENV}", std::env::consts::EXE_SUFFIX));
        }
        assert_eq!(reason(&Ok(Found::Embedded { unpacked: false })), None);
        assert_eq!(reason(&Err(Failure::Unpack("disk full".into()))).unwrap(), "OpenCascade's worker would not unpack: disk full");
        assert!(root().is_absolute(), "{}", root().display());
    }
}
