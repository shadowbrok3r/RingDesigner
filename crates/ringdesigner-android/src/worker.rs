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

    fn ms<T>(f: impl FnOnce() -> T) -> (f64, T) {
        let t = std::time::Instant::now();
        let out = f();
        (t.elapsed().as_secs_f64() * 1e3, out)
    }

    /// The UI thread's cost of what the phone's landing, Files > Open, save, commit, undo and redo call, on the heaviest templates, timed on the host: `cargo test --release -p ringdesigner_android phone_ui_thread_costs -- --ignored --nocapture`.
    #[test]
    #[ignore = "a timing table for release builds"]
    fn phone_ui_thread_costs_on_heavy_designs() {
        use ringdesign_workbench::templates::{Polled, Slot, collections, open_file};
        let reg = Arc::new(ringdesign_script::registry());
        let lib = Arc::new(ringdesign_core::AlphaLibrary::installed());
        let dir = std::env::temp_dir().join(format!("phone-costs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut rows: Vec<(String, f64)> = Vec::new();
        let land = |slot: &mut Slot<bool>, rows: &mut Vec<(String, f64)>, what: String| {
            let started = std::time::Instant::now();
            loop {
                let (t, polled) = ms(|| slot.poll(&lib));
                match polled {
                    Polled::Landed(landing) => {
                        rows.push((what, t));
                        break landing;
                    }
                    Polled::Failed(why) => panic!("{why}"),
                    _ => assert!(started.elapsed().as_secs() < 120, "{what} never landed"),
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        };
        for slug in ["caiman-imported", "nocturne"] {
            let template = collections().iter().flat_map(|c| &c.templates).find(|t| t.slug == slug).unwrap();
            let mut slot = Slot::default();
            let (t, _) = ms(|| slot.start(template.open(reg.clone(), lib.clone(), || {}), true));
            rows.push((format!("{slug}: template chosen"), t));
            let landing = land(&mut slot, &mut rows, format!("{slug}: template landed, baked onto the library"));
            let mut graph = crate::graph::GraphState::new();
            let (t, _) = ms(|| graph.sync(&landing.design));
            rows.push((format!("{slug}: graph sync on landing"), t));
            let (t, _) = ms(|| graph.ed.as_mut().map(|ed| ed.arrange(&graph.reg)));
            rows.push((format!("{slug}: arrange on landing"), t));
            // What that sync spends it on, and the same sync again in a fresh state.
            let json = landing.design.graph.clone().expect("a graph template");
            let (t, _) = ms(|| serde_json::from_value::<ringdesign_graph::graph::Graph>(json.clone()).unwrap());
            rows.push((format!("{slug}:   graph read from a copy of its JSON, as graph.rs does"), t));
            let (t, parsed) = ms(|| <ringdesign_graph::graph::Graph as serde::Deserialize>::deserialize(&json).unwrap());
            rows.push((format!("{slug}:   graph read from its JSON in place"), t));
            let (t, mut ed) = ms(|| ringdesign_graph_ui::Editor::new(parsed, &graph.reg));
            rows.push((format!("{slug}:   editor built"), t));
            let (t, _) = ms(|| ed.fit());
            rows.push((format!("{slug}:   editor fitted"), t));
            let (t, _) = ms(|| ed.arrange_if_tangled());
            rows.push((format!("{slug}:   arranged if tangled"), t));
            let (t, _) = ms(|| crate::graph::GraphState::new().sync(&landing.design));
            rows.push((format!("{slug}:   the whole sync again, fresh state"), t));
            let the_graph = landing.graph.clone().expect("read on the opening thread");
            rows.push((format!("{slug}:   graph already read on the opening thread: {} nodes", the_graph.nodes.len()), 0.0));
            let (t, _) = ms(|| graph.changed(&mut landing.design.clone()));
            rows.push((format!("{slug}: graph written back after a move (with a design copy)"), t));
            let mut history = ringdesign_core::history::History::new(&RingDesign::default());
            let (t, _) = ms(|| history.reset(&landing.design));
            rows.push((format!("{slug}: history reset on adopt"), t));
            let (t, _) = ms(|| ringdesign_graph::eval::baked(&landing.design, &landing.lib));
            rows.push((format!("{slug}: adopt's bake over the landed library"), t));
            let (t, copy) = ms(|| landing.design.clone());
            rows.push((format!("{slug}: save, autosave, save a copy: the design handed to the writer"), t));
            let path = dir.join(format!("{slug}.ring.json"));
            library::save_design_embedded(&path, &copy, &landing.lib).unwrap();
            let (t, _) = ms(|| {
                let d = library::load_design(&path).unwrap();
                ringdesign_graph::eval::baked(&d, &lib)
            });
            rows.push((format!("{slug}: Files > Open as it was, read and baked on the UI thread"), t));
            let (t, _) = ms(|| slot.start(open_file(path.clone(), lib.clone(), false, || {}), false));
            rows.push((format!("{slug}: Files > Open now, started"), t));
            let opened = land(&mut slot, &mut rows, format!("{slug}: Files > Open now, landed"));
            let mut edited = opened.design.clone();
            edited.name.push_str(" edited");
            let (t, _) = ms(|| history.commit(&edited));
            rows.push((format!("{slug}: history commit of a settled edit"), t));
            let (t, _) = ms(|| history.undo().map(|d| ringdesign_graph::eval::baked(&d, &opened.lib)));
            rows.push((format!("{slug}: undo, with its bake"), t));
            let (t, _) = ms(|| history.redo().map(|d| ringdesign_graph::eval::baked(&d, &opened.lib)));
            rows.push((format!("{slug}: redo, with its bake"), t));
        }
        let _ = std::fs::remove_dir_all(&dir);
        println!("| phone UI thread, on the host | ms |\n| --- | --- |");
        for (what, t) in rows {
            println!("| {what} | {t:.1} |");
        }
    }
}
