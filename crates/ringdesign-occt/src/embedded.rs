//! A worker carried inside its host's executable: deflated when the host is built, unpacked once into a data folder, checked by its SHA-256.
use crate::client::{Failure, WORKER_NAME};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

/// Bytes read or written at a time while inflating and hashing.
const CHUNK: usize = 1 << 20;

/// A deflated worker program and the digest and length of what it inflates to.
#[derive(Clone, Copy, Debug)]
pub struct Embedded {
    /// The program as a raw deflate stream; empty when the host carries none.
    pub deflated: &'static [u8],
    /// Lower-case hex SHA-256 of the inflated program.
    pub sha256: &'static str,
    /// Its length inflated, bytes.
    pub bytes: u64,
}

/// A worker taken from its host: where it sits, whether this call inflated it, and what that took.
#[derive(Clone, Debug, PartialEq)]
pub struct Unpacked {
    pub path: PathBuf,
    pub fresh: bool,
    pub took: Duration,
}

/// Copies whose digest this process has read, by path, modification time and length.
static CHECKED: Mutex<Vec<(PathBuf, Option<SystemTime>, u64)>> = Mutex::new(Vec::new());
/// One unpack at a time in this process.
static UNPACKING: Mutex<()> = Mutex::new(());

impl Embedded {
    /// A host that carries no worker.
    pub const NONE: Self = Self { deflated: &[], sha256: "", bytes: 0 };

    /// Whether there is a program here and a digest to check it by.
    pub fn is_present(&self) -> bool {
        !self.deflated.is_empty() && self.sha256.len() == 64 && self.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    }

    /// Where it unpacks under `root`: `occt/<sha256>/occt-worker`, with the platform's executable suffix.
    pub fn path_under(&self, root: &Path) -> PathBuf {
        root.join("occt").join(self.sha256).join(format!("{WORKER_NAME}{}", std::env::consts::EXE_SUFFIX))
    }

    /// Whether a copy of the right length already sits under `root`.
    pub fn is_unpacked(&self, root: &Path) -> bool {
        self.is_present() && std::fs::metadata(self.path_under(root)).is_ok_and(|m| m.is_file() && m.len() == self.bytes)
    }

    /// The worker under `root`: a copy whose length and digest hold is reused, else the program is inflated beside it, checked, made executable and moved into place.
    pub fn unpack(&self, root: &Path) -> Result<Unpacked, Failure> {
        let started = Instant::now();
        if !self.is_present() {
            return Err(Failure::Unpack("this build carries no OpenCascade worker".into()));
        }
        let path = self.path_under(root);
        let _one = UNPACKING.lock().unwrap_or_else(|e| e.into_inner());
        if self.is_unpacked(root) && (checked(&path) || digest_of(&path).is_ok_and(|d| d == self.sha256)) {
            remember(&path);
            return Ok(Unpacked { path, fresh: false, took: started.elapsed() });
        }
        let dir = path.parent().ok_or_else(|| Failure::Unpack(format!("{} has no folder", path.display())))?;
        std::fs::create_dir_all(dir).map_err(|e| Failure::Unpack(format!("{}: {e}", dir.display())))?;
        let part = dir.join(format!("{WORKER_NAME}.{}.part", std::process::id()));
        let written = self.inflate_to(&part);
        if let Err(e) = written {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
        if let Err(e) = std::fs::rename(&part, &path) {
            let _ = std::fs::remove_file(&part);
            // Another process that moved its own copy into place first leaves one that holds.
            if !(self.is_unpacked(root) && digest_of(&path).is_ok_and(|d| d == self.sha256)) {
                return Err(Failure::Unpack(format!("{}: {e}", path.display())));
            }
        }
        remember(&path);
        self.prune(root);
        Ok(Unpacked { path, fresh: true, took: started.elapsed() })
    }

    /// Removes the copies earlier builds unpacked beside this one, best effort: a copy still running stays.
    fn prune(&self, root: &Path) {
        let Ok(entries) = std::fs::read_dir(root.join("occt")) else { return };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name != self.sha256 && name.len() == 64 && name.bytes().all(|b| b.is_ascii_hexdigit()) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    /// The program inflated into `part`, its length and digest checked and its executable bit set.
    fn inflate_to(&self, part: &Path) -> Result<(), Failure> {
        let fail = |e: std::io::Error| Failure::Unpack(format!("{}: {e}", part.display()));
        let mut file = std::fs::File::create(part).map_err(fail)?;
        let mut inflate = flate2::read::DeflateDecoder::new(self.deflated);
        let (mut hash, mut length, mut buffer) = (Sha256::new(), 0u64, vec![0u8; CHUNK]);
        loop {
            let n = inflate.read(&mut buffer).map_err(|e| Failure::Unpack(format!("the embedded worker does not inflate: {e}")))?;
            if n == 0 {
                break;
            }
            length += n as u64;
            if length > self.bytes {
                return Err(Failure::Unpack(format!("the embedded worker inflates past its {} bytes", self.bytes)));
            }
            hash.update(&buffer[..n]);
            file.write_all(&buffer[..n]).map_err(fail)?;
        }
        file.sync_all().map_err(fail)?;
        drop(file);
        let digest = hex(&hash.finalize());
        if length != self.bytes || digest != self.sha256 {
            return Err(Failure::Unpack(format!("the embedded worker inflates to {length} bytes with SHA-256 {digest}, not {} bytes with {}", self.bytes, self.sha256)));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(part, std::fs::Permissions::from_mode(0o755)).map_err(fail)?;
        }
        Ok(())
    }
}

/// Lower-case hex of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The SHA-256 of the file at `path`, as lower-case hex.
pub fn digest_of(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let (mut hash, mut buffer) = (Sha256::new(), vec![0u8; CHUNK]);
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            return Ok(hex(&hash.finalize()));
        }
        hash.update(&buffer[..n]);
    }
}

/// What identifies a copy as the one read before: its modification time and length.
fn stamp(path: &Path) -> Option<(Option<SystemTime>, u64)> {
    std::fs::metadata(path).ok().map(|m| (m.modified().ok(), m.len()))
}

/// Whether this process has read `path`'s digest since it last changed.
fn checked(path: &Path) -> bool {
    let Some((modified, len)) = stamp(path) else { return false };
    CHECKED.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|(p, m, l)| p == path && *m == modified && *l == len)
}

/// Records `path` as read, as it stands now.
fn remember(path: &Path) {
    let Some((modified, len)) = stamp(path) else { return };
    let mut seen = CHECKED.lock().unwrap_or_else(|e| e.into_inner());
    seen.retain(|(p, ..)| p != path);
    seen.push((path.to_path_buf(), modified, len));
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::client::Worker;
    use crate::protocol::{MARKER, Request, Response};

    /// A shell script standing in for the worker, deflated as a host's build script packs one.
    fn payload(notes: &str) -> (Embedded, Vec<u8>) {
        let body = format!("#!/bin/sh\ncat > /dev/null\necho '{MARKER}'\necho '{{\"outcome\":\"done\",\"solids\":[],\"notes\":[\"{notes}\"],\"kernel_ms\":0.0}}'\n");
        let raw = body.into_bytes();
        let mut z = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
        z.write_all(&raw).unwrap();
        let deflated: &'static [u8] = Box::leak(z.finish().unwrap().into_boxed_slice());
        let sha256: &'static str = Box::leak(hex(&Sha256::digest(&raw)).into_boxed_str());
        (Embedded { deflated, sha256, bytes: raw.len() as u64 }, raw)
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ringdesign-occt-embedded-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn an_embedded_worker_unpacks_once_runs_and_is_reused() {
        use std::os::unix::fs::PermissionsExt;
        let (worker, raw) = payload("embedded");
        let root = scratch("once");
        assert!(worker.is_present() && !Embedded::NONE.is_present() && !worker.is_unpacked(&root));
        let first = worker.unpack(&root).unwrap();
        assert_eq!((first.fresh, &first.path), (true, &root.join("occt").join(worker.sha256).join("occt-worker")));
        assert_eq!(std::fs::read(&first.path).unwrap(), raw);
        assert_eq!(std::fs::metadata(&first.path).unwrap().permissions().mode() & 0o777, 0o755);
        let answer = Worker::at(&first.path).run(&Request::Ping, Duration::from_secs(5)).unwrap();
        assert_eq!(answer, Response::Done { solids: Vec::new(), notes: vec!["embedded".into()], kernel_ms: 0.0 });
        let again = worker.unpack(&root).unwrap();
        assert_eq!((again.fresh, again.path), (false, first.path.clone()));
        // A newer build's worker takes the older copy's place and leaves anything else in the folder alone.
        let (newer, _) = payload("newer");
        std::fs::create_dir_all(root.join("occt").join("notes")).unwrap();
        let replaced = newer.unpack(&root).unwrap();
        assert!(replaced.fresh && !first.path.exists() && root.join("occt").join("notes").is_dir(), "{replaced:?}");
        let answer = Worker::at(&replaced.path).run(&Request::Ping, Duration::from_secs(5)).unwrap();
        assert_eq!(answer, Response::Done { solids: Vec::new(), notes: vec!["newer".into()], kernel_ms: 0.0 });
        // Nothing but the worker is left in its folder.
        let left: Vec<_> = std::fs::read_dir(replaced.path.parent().unwrap()).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left, ["occt-worker"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn find_falls_back_on_the_embedded_worker_and_probe_says_so_without_unpacking() {
        use crate::client::{Found, WORKER_ENV, probe};
        if std::env::var_os(WORKER_ENV).is_some() {
            eprintln!("{WORKER_ENV} is set; the fallback is not reached");
            return;
        }
        let (worker, _) = payload("found");
        let root = scratch("find");
        let none = probe(&Embedded::NONE, &root);
        assert!(matches!(&none, Err(Failure::Missing(p)) if p.ends_with("occt-worker")), "{none:?}");
        assert_eq!(probe(&worker, &root).unwrap(), Found::Embedded { unpacked: false });
        assert!(!root.exists(), "a probe unpacks nothing");
        let found = Worker::find(&worker, &root).unwrap();
        assert_eq!(found, Worker::at(worker.path_under(&root)));
        assert_eq!(probe(&worker, &root).unwrap(), Found::Embedded { unpacked: true });
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_damaged_copy_is_unpacked_again_and_a_wrong_digest_is_refused() {
        let (worker, raw) = payload("mended");
        let root = scratch("damaged");
        let path = worker.unpack(&root).unwrap().path;
        // The same length with one byte changed, a second later so its time differs from the copy read.
        std::thread::sleep(Duration::from_millis(1100));
        let mut bad = raw.clone();
        bad[20] ^= 0x20;
        std::fs::write(&path, &bad).unwrap();
        let mended = worker.unpack(&root).unwrap();
        assert!(mended.fresh, "{mended:?}");
        assert_eq!(std::fs::read(&path).unwrap(), raw);
        // A payload whose digest names something else leaves nothing behind.
        let lying = Embedded { sha256: "0".repeat(64).leak(), ..worker };
        let refused = lying.unpack(&root);
        assert!(matches!(&refused, Err(Failure::Unpack(m)) if m.contains("not ")), "{refused:?}");
        let dir = lying.path_under(&root).parent().unwrap().to_path_buf();
        assert_eq!(std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0), 0);
        let none = Embedded::NONE.unpack(&root);
        assert!(matches!(&none, Err(Failure::Unpack(m)) if m == "this build carries no OpenCascade worker"), "{none:?}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
