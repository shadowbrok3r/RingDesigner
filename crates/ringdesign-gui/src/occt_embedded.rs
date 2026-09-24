//! The OpenCascade worker this build carries, if any, where it unpacks, and whether the app has a worker at all.
use ringdesign_occt::client::{Failure, Found, Locator, WORKER_ENV, WORKER_NAME, Worker};
use ringdesign_occt::embedded::Embedded;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

include!(concat!(env!("OUT_DIR"), "/occt_worker.rs"));

/// The folder an embedded worker unpacks under: the library's data root, or on Windows the local, unroamed app data.
pub fn root() -> PathBuf {
    let local = std::env::var_os("LOCALAPPDATA").filter(|_| cfg!(windows)).map(|d| PathBuf::from(d).join("RingDesigner"));
    let data = Some(ringdesign_core::library::data_root()).filter(|d| d.is_absolute());
    local.or(data).unwrap_or_else(|| std::env::temp_dir().join("ringdesigner"))
}

/// Where the app looks for its worker, read from the environment once: a named worker, one beside the app, the one it carries.
pub fn locator() -> Locator {
    Locator::from_env(EMBEDDED, root())
}

/// Why OpenCascade cannot be offered, or `None` when `locator` finds a worker.
pub fn missing(locator: &Locator) -> Option<String> {
    reason(locator, &locator.probe())
}

/// What a probe says to someone deciding whether to use OpenCascade.
pub fn reason(locator: &Locator, probe: &Result<Found, Failure>) -> Option<String> {
    let exe = format!("{WORKER_NAME}{}", std::env::consts::EXE_SUFFIX);
    match (probe, &locator.named) {
        (Ok(_), _) => None,
        (Err(Failure::Missing(p)), Some((var, _))) => Some(format!("{var} names {}, which is not there", p.display())),
        (Err(Failure::Missing(_)), None) => Some(format!("This build carries no OpenCascade worker: put {exe} beside RingDesigner, or name one in {WORKER_ENV}")),
        (Err(e), _) => Some(e.to_string()),
    }
}

/// The worker `locator` finds, the embedded one unpacked on its first use unless `cancel` stops it; the wait is logged.
pub fn worker(locator: &Locator, cancel: &AtomicBool) -> Result<Worker, String> {
    let fresh = unpacks_first(locator);
    let started = std::time::Instant::now();
    let worker = locator.find_cancellable(cancel).map_err(|e| e.to_string())?;
    if fresh {
        log::info!("OpenCascade's worker unpacked to {} in {:.0} ms", worker.program.display(), started.elapsed().as_secs_f64() * 1e3);
    }
    Ok(worker)
}

/// Whether the next run first unpacks the worker this build carries.
pub fn unpacks_first(locator: &Locator) -> bool {
    matches!(locator.probe(), Ok(Found::Embedded { unpacked: false }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_worker_says_where_one_would_go() {
        let beside = Err(Failure::Missing(PathBuf::from("/opt/ringdesigner/occt-worker")));
        let named = Locator::named("/opt/ringdesigner/occt-worker");
        assert_eq!(reason(&named, &beside).unwrap(), "RINGDESIGN_OCCT_WORKER names /opt/ringdesigner/occt-worker, which is not there");
        let alias = Locator { named: Some((ringdesign_occt::client::WORKER_ENV_ALIAS, "/opt/w".into())), ..Locator::nowhere() };
        assert_eq!(missing(&alias).unwrap(), "RINGDESIGNER_OCCT_WORKER names /opt/w, which is not there");
        let said = reason(&Locator::nowhere(), &beside).unwrap();
        assert_eq!(said, format!("This build carries no OpenCascade worker: put occt-worker{} beside RingDesigner, or name one in RINGDESIGN_OCCT_WORKER", std::env::consts::EXE_SUFFIX));
        assert_eq!(missing(&Locator::nowhere()), Some(said));
        assert_eq!(reason(&Locator::nowhere(), &Ok(Found::Embedded { unpacked: false })), None);
        assert_eq!(reason(&Locator::nowhere(), &Err(Failure::Unpack("disk full".into()))).unwrap(), "OpenCascade's worker would not unpack: disk full");
        assert!(root().is_absolute(), "{}", root().display());
    }
}
