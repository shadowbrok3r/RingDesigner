//! Design files written off the UI thread: the newest queued write of a path wins, and a flush of a path waits for the write in flight and stands over any queued before it.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};

use ringdesign_core::{RingDesign, library};

/// What a write was for, handed back with its result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Autosave,
    Save,
    Downloads,
}

/// A write that landed.
#[derive(Debug)]
pub struct Saved {
    pub path: PathBuf,
    pub purpose: Purpose,
    pub result: Result<(), String>,
}

struct Write {
    path: PathBuf,
    design: RingDesign,
    purpose: Purpose,
    queued: u64,
}

/// What the writer thread and the flushing thread share: the lock a write holds, the queue's clock, and the clock at each path's last flush.
#[derive(Default)]
struct Shared {
    writing: Mutex<HashMap<PathBuf, u64>>,
    clock: AtomicU64,
}

/// One writer thread for design files.
pub struct Saver {
    jobs: Sender<Write>,
    done: Receiver<Saved>,
    shared: Arc<Shared>,
}

impl Saver {
    /// A writer that calls `wake` as each write lands.
    pub fn spawn(wake: impl Fn() + Send + 'static) -> Self {
        let (jobs, rx) = mpsc::channel::<Write>();
        let (tx, done) = mpsc::channel();
        let shared = Arc::new(Shared::default());
        let held = shared.clone();
        let _ = std::thread::Builder::new().name("design-save".into()).spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut queued = vec![first];
                while let Ok(next) = rx.try_recv() {
                    queued.retain(|w| w.path != next.path);
                    queued.push(next);
                }
                for w in queued {
                    let result = {
                        let flushed = held.writing.lock().unwrap_or_else(|e| e.into_inner());
                        match flushed.get(&w.path) {
                            Some(&at) if at > w.queued => Ok(()),
                            _ => library::save_design(&w.path, &w.design).map_err(|e| format!("{e:#}")),
                        }
                    };
                    if tx.send(Saved { path: w.path, purpose: w.purpose, result }).is_err() {
                        return;
                    }
                    wake();
                }
            }
        });
        Self { jobs, done, shared }
    }

    /// Queues `design` to be written to `path`; a later write of the same path queued before it starts replaces it.
    pub fn save(&self, path: PathBuf, design: RingDesign, purpose: Purpose) {
        let queued = self.shared.clock.fetch_add(1, Ordering::Relaxed);
        let _ = self.jobs.send(Write { path, design, purpose, queued });
    }

    /// A write that has landed.
    pub fn poll(&self) -> Option<Saved> {
        match self.done.try_recv() {
            Ok(saved) => Some(saved),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }

    /// Writes `design` to `path` on the calling thread, after any write in flight; writes of `path` queued before it are then skipped.
    pub fn flush(&self, path: &Path, design: &RingDesign) -> Result<(), String> {
        let mut flushed = self.shared.writing.lock().unwrap_or_else(|e| e.into_inner());
        flushed.insert(path.to_path_buf(), self.shared.clock.fetch_add(1, Ordering::Relaxed));
        library::save_design(path, design).map_err(|e| format!("{e:#}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_name(path: &Path) -> String {
        library::load_design(path).unwrap().name
    }

    fn design(name: &str) -> RingDesign {
        RingDesign { name: name.into(), ..RingDesign::default() }
    }

    #[test]
    fn the_newest_write_of_a_path_lands_and_each_says_what_it_was_for() {
        let dir = std::env::temp_dir().join(format!("saver-newest-{}", std::process::id()));
        let (auto, saved) = (dir.join("autosave.ring.json"), dir.join("mine.ring.json"));
        let saver = Saver::spawn(|| {});
        for k in 0..6 {
            saver.save(auto.clone(), design(&format!("edit {k}")), Purpose::Autosave);
        }
        saver.save(saved.clone(), design("saved"), Purpose::Save);
        let mut landed: Vec<Saved> = Vec::new();
        let start = std::time::Instant::now();
        while !landed.iter().any(|s| s.purpose == Purpose::Save) || !(auto.exists() && read_name(&auto) == "edit 5") {
            landed.extend(saver.poll());
            assert!(start.elapsed().as_secs() < 30, "{landed:?}");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(landed.iter().all(|s| s.result.is_ok()), "{landed:?}");
        assert_eq!(read_name(&saved), "saved");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_flush_stands_over_every_write_of_its_path_queued_before_it() {
        let dir = std::env::temp_dir().join(format!("saver-flush-{}", std::process::id()));
        let auto = dir.join("autosave.ring.json");
        let saver = Saver::spawn(|| {});
        for k in 0..20 {
            saver.save(auto.clone(), design(&format!("queued {k}")), Purpose::Autosave);
        }
        saver.flush(&auto, &design("flushed")).unwrap();
        // A write queued after everything else lands after everything else.
        let sentinel = dir.join("sentinel.ring.json");
        saver.save(sentinel.clone(), design("sentinel"), Purpose::Save);
        let start = std::time::Instant::now();
        while !saver.poll().is_some_and(|s| s.path == sentinel) {
            assert!(start.elapsed().as_secs() < 30, "the sentinel never landed");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(read_name(&auto), "flushed", "nothing queued before the flush wrote over it");
        saver.save(auto.clone(), design("after"), Purpose::Autosave);
        let start = std::time::Instant::now();
        while read_name(&auto) != "after" {
            assert!(start.elapsed().as_secs() < 30);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
