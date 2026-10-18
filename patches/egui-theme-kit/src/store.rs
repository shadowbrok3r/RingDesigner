//! Theme persistence: the [`ThemeStore`] contract, a debounced [`AutoSave`], and in-memory, worker-backed, directory and eframe-storage stores.
//!
//! Every [`ThemeStore`] method is synchronous and never waits on I/O: reads come from a cache and
//! writes update the cache at once and reach the backing store later. [`ThemeStore::revision`]
//! increases on every visible change so a UI knows when to refresh its lists.
//!
//! # Storing themes in an application database
//!
//! Implement [`ThemeBackend`], the blocking half, and wrap it in [`CachedStore::open`]; the
//! wrapper keeps the cache and runs the backend on a worker thread, so an async client is driven
//! with `block_on` inside the backend and the UI thread never awaits.
//!
//! ```ignore
//! use egui_theme_kit::store::{CachedStore, StoreOp, StoreSnapshot, ThemeBackend};
//!
//! struct DbBackend {
//!     db: Surreal<Any>,
//!     rt: tokio::runtime::Handle,
//! }
//!
//! impl ThemeBackend for DbBackend {
//!     fn load(&mut self) -> Result<StoreSnapshot, String> {
//!         let rows: Vec<ThemeRow> = self.rt
//!             .block_on(self.db.select("ui_theme"))
//!             .map_err(|e| e.to_string())?;
//!         Ok(snapshot_from_rows(rows))
//!     }
//!
//!     fn apply(&mut self, op: &StoreOp) -> Result<(), String> {
//!         match op {
//!             StoreOp::SaveActive(theme) => self.upsert("active", theme),
//!             StoreOp::SaveUser(named) => self.upsert(&named.name, &named.theme),
//!             StoreOp::DeleteUser(name) => self.remove(name),
//!             _ => Ok(()),
//!         }
//!     }
//! }
//!
//! let store = std::sync::Arc::new(CachedStore::open(DbBackend { db, rt }));
//! let (theme, source) = egui_theme_kit::persist::resolve_with_store(&*store, None);
//! ```
//!
//! Apps that persist through `eframe` storage use [`EframeStorageStore`] instead.

use std::fmt;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui::{Context, Style};
use serde::{Deserialize, Serialize};

use super::{
    glass::GlassParams,
    persist::{self, ThemeSource},
    presets,
    saved::{SavedTheme, SemanticColors, theme_from_json, theme_to_json},
};

/// Longest theme name, in characters.
pub const MAX_NAME_CHARS: usize = 64;

/// A theme a user saved under a name.
#[derive(Clone, Debug)]
pub struct NamedTheme {
    pub name: String,
    pub theme: SavedTheme,
    /// Seconds since the Unix epoch when the theme was last saved.
    pub updated_at: u64,
}

/// Which theme an app applies at startup.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "name", rename_all = "snake_case")]
pub enum StartupTheme {
    /// The theme that was active when the app last ran.
    #[default]
    LastUsed,
    /// A saved user theme, by name.
    User(String),
    /// A preset, by id.
    Preset(String),
}

/// Why a theme name was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
    Control,
}

impl fmt::Display for NameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("enter a name"),
            Self::TooLong => write!(f, "names are at most {MAX_NAME_CHARS} characters"),
            Self::Control => f.write_str("names cannot contain control characters"),
        }
    }
}

impl std::error::Error for NameError {}

/// `name` with runs of whitespace collapsed to single spaces, or why it is not usable.
pub fn clean_name(name: &str) -> Result<String, NameError> {
    if name.chars().any(|c| c.is_control() && !c.is_whitespace()) {
        return Err(NameError::Control);
    }
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    match name.chars().count() {
        0 => Err(NameError::Empty),
        n if n > MAX_NAME_CHARS => Err(NameError::TooLong),
        _ => Ok(name),
    }
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Persistent theme storage that never blocks the caller.
pub trait ThemeStore: Send + Sync {
    /// The theme that was active when the store last saved, if any.
    fn load_active(&self) -> Option<SavedTheme>;
    /// Records `theme` as the active theme.
    fn save_active(&self, theme: &SavedTheme);
    /// Forgets the active theme so the next start uses the default.
    fn clear_active(&self);
    /// Saved user themes sorted by name.
    fn user_themes(&self) -> Vec<NamedTheme>;
    /// Saves `theme` under `name`, replacing a theme of that name.
    fn save_user_theme(&self, name: &str, theme: &SavedTheme);
    fn delete_user_theme(&self, name: &str);
    /// Renames a theme; fails with [`ThemeStore::last_error`] when `to` is taken.
    fn rename_user_theme(&self, from: &str, to: &str);
    /// The theme pinned for startup.
    fn startup(&self) -> StartupTheme;
    fn set_startup(&self, startup: StartupTheme);
    /// A counter that increases whenever the visible state changes.
    fn revision(&self) -> u64;
    /// The latest failure of a store operation, until cleared.
    fn last_error(&self) -> Option<String> {
        None
    }
    fn clear_error(&self) {}
}

/// A change to the stored state, in the form a backend applies.
#[derive(Clone, Debug)]
pub enum StoreOp {
    SaveActive(SavedTheme),
    ClearActive,
    SaveUser(NamedTheme),
    DeleteUser(String),
    RenameUser { from: String, to: String },
    SetStartup(StartupTheme),
}

/// Everything a backend holds, as read at startup or on reload.
#[derive(Clone, Debug, Default)]
pub struct StoreSnapshot {
    pub active: Option<SavedTheme>,
    pub user_themes: Vec<NamedTheme>,
    pub startup: StartupTheme,
    /// Records that could not be read and were skipped.
    pub warnings: Vec<String>,
}

#[derive(Default)]
struct State {
    active: Option<SavedTheme>,
    user: Vec<NamedTheme>,
    startup: StartupTheme,
    error: Option<String>,
}

impl State {
    fn from_snapshot(snapshot: StoreSnapshot) -> Self {
        let mut user = snapshot.user_themes;
        sort_themes(&mut user);
        Self {
            active: snapshot.active,
            user,
            startup: snapshot.startup,
            error: (!snapshot.warnings.is_empty()).then(|| snapshot.warnings.join("; ")),
        }
    }

    fn position(&self, name: &str) -> Option<usize> {
        self.user.iter().position(|t| same_name(&t.name, name))
    }

    /// Applies `op`; returns the normalized ops to forward to a backend, none when nothing changed.
    fn apply(&mut self, op: StoreOp) -> Result<Vec<StoreOp>, String> {
        match op {
            StoreOp::SaveActive(theme) => {
                self.active = Some(theme.clone());
                Ok(vec![StoreOp::SaveActive(theme)])
            }
            StoreOp::ClearActive => {
                self.active = None;
                Ok(vec![StoreOp::ClearActive])
            }
            StoreOp::SaveUser(mut named) => {
                named.name = clean_name(&named.name).map_err(|e| e.to_string())?;
                match self.position(&named.name) {
                    Some(i) => self.user[i] = named.clone(),
                    None => self.user.push(named.clone()),
                }
                sort_themes(&mut self.user);
                Ok(vec![StoreOp::SaveUser(named)])
            }
            StoreOp::DeleteUser(name) => {
                let Some(i) = self.position(&name) else {
                    return Ok(Vec::new());
                };
                let removed = self.user.remove(i);
                let mut ops = vec![StoreOp::DeleteUser(removed.name.clone())];
                if matches!(&self.startup, StartupTheme::User(n) if same_name(n, &removed.name)) {
                    self.startup = StartupTheme::LastUsed;
                    ops.push(StoreOp::SetStartup(StartupTheme::LastUsed));
                }
                Ok(ops)
            }
            StoreOp::RenameUser { from, to } => {
                let to = clean_name(&to).map_err(|e| e.to_string())?;
                let i = self
                    .position(&from)
                    .ok_or_else(|| format!("there is no theme named \"{from}\""))?;
                if self.user[i].name == to {
                    return Ok(Vec::new());
                }
                if self.position(&to).is_some_and(|j| j != i) {
                    return Err(format!("a theme named \"{to}\" already exists"));
                }
                let old = std::mem::replace(&mut self.user[i].name, to.clone());
                let mut ops = vec![StoreOp::RenameUser {
                    from: old.clone(),
                    to: to.clone(),
                }];
                if matches!(&self.startup, StartupTheme::User(n) if same_name(n, &old)) {
                    self.startup = StartupTheme::User(to.clone());
                    ops.push(StoreOp::SetStartup(self.startup.clone()));
                }
                sort_themes(&mut self.user);
                Ok(ops)
            }
            StoreOp::SetStartup(startup) => {
                let startup = match startup {
                    StartupTheme::LastUsed => StartupTheme::LastUsed,
                    StartupTheme::User(name) => {
                        let i = self
                            .position(&name)
                            .ok_or_else(|| format!("there is no theme named \"{name}\""))?;
                        StartupTheme::User(self.user[i].name.clone())
                    }
                    StartupTheme::Preset(label) => {
                        let def = presets::find_preset(&label)
                            .ok_or_else(|| format!("there is no preset \"{label}\""))?;
                        StartupTheme::Preset(def.id.to_owned())
                    }
                };
                if self.startup == startup {
                    return Ok(Vec::new());
                }
                self.startup = startup.clone();
                Ok(vec![StoreOp::SetStartup(startup)])
            }
        }
    }
}

fn sort_themes(themes: &mut [NamedTheme]) {
    themes.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.name.cmp(&b.name))
    });
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The state and revision counter every store shares.
struct Cache {
    state: Mutex<State>,
    revision: AtomicU64,
}

impl Cache {
    fn new(state: State) -> Self {
        Self {
            state: Mutex::new(state),
            revision: AtomicU64::new(1),
        }
    }

    fn bump(&self) {
        self.revision.fetch_add(1, Ordering::AcqRel);
    }

    /// Applies `op` to the cache; records a failure as the last error and returns the ops to forward.
    fn submit(&self, op: StoreOp) -> Vec<StoreOp> {
        let result = lock(&self.state).apply(op);
        let forwarded = match result {
            Ok(forwarded) => forwarded,
            Err(message) => {
                lock(&self.state).error = Some(message);
                Vec::new()
            }
        };
        self.bump();
        forwarded
    }

    fn set_error(&self, message: String) {
        lock(&self.state).error = Some(message);
        self.bump();
    }

    fn replace(&self, snapshot: StoreSnapshot) {
        *lock(&self.state) = State::from_snapshot(snapshot);
        self.bump();
    }

    fn clear_error(&self) {
        if lock(&self.state).error.take().is_some() {
            self.bump();
        }
    }
}

/// Implements the read half of [`ThemeStore`] over a `cache` field.
macro_rules! cache_reads {
    () => {
        fn load_active(&self) -> Option<SavedTheme> {
            lock(&self.cache.state).active.clone()
        }

        fn user_themes(&self) -> Vec<NamedTheme> {
            lock(&self.cache.state).user.clone()
        }

        fn startup(&self) -> StartupTheme {
            lock(&self.cache.state).startup.clone()
        }

        fn revision(&self) -> u64 {
            self.cache.revision.load(Ordering::Acquire)
        }

        fn last_error(&self) -> Option<String> {
            lock(&self.cache.state).error.clone()
        }

        fn clear_error(&self) {
            self.cache.clear_error();
        }
    };
}

fn named(name: &str, theme: &SavedTheme) -> NamedTheme {
    NamedTheme {
        name: name.to_owned(),
        theme: theme.clone(),
        updated_at: now_secs(),
    }
}

/// A store that keeps everything in memory; for tests and apps that persist elsewhere.
pub struct MemoryStore {
    cache: Cache,
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self {
            cache: Cache::new(State::default()),
        }
    }
}

impl MemoryStore {
    /// A store starting from `snapshot`.
    pub fn with_snapshot(snapshot: StoreSnapshot) -> Self {
        Self {
            cache: Cache::new(State::from_snapshot(snapshot)),
        }
    }
}

impl ThemeStore for MemoryStore {
    cache_reads!();

    fn save_active(&self, theme: &SavedTheme) {
        self.cache.submit(StoreOp::SaveActive(theme.clone()));
    }

    fn clear_active(&self) {
        self.cache.submit(StoreOp::ClearActive);
    }

    fn save_user_theme(&self, name: &str, theme: &SavedTheme) {
        self.cache.submit(StoreOp::SaveUser(named(name, theme)));
    }

    fn delete_user_theme(&self, name: &str) {
        self.cache.submit(StoreOp::DeleteUser(name.to_owned()));
    }

    fn rename_user_theme(&self, from: &str, to: &str) {
        self.cache.submit(StoreOp::RenameUser {
            from: from.to_owned(),
            to: to.to_owned(),
        });
    }

    fn set_startup(&self, startup: StartupTheme) {
        self.cache.submit(StoreOp::SetStartup(startup));
    }
}

/// The blocking half of a store: reads everything once and applies one change at a time.
pub trait ThemeBackend: Send + 'static {
    /// Reads every stored record.
    fn load(&mut self) -> Result<StoreSnapshot, String>;
    /// Persists one change; ops arrive in order, validated and with names cleaned.
    fn apply(&mut self, op: &StoreOp) -> Result<(), String>;
}

enum Msg {
    Op(Box<StoreOp>),
    Reload,
    Flush(mpsc::Sender<()>),
    Shutdown,
}

struct Shared {
    cache: Cache,
    pending: AtomicUsize,
}

/// A [`ThemeStore`] over a [`ThemeBackend`]: a cache for the UI thread and a worker thread for the backend.
pub struct CachedStore {
    shared: Arc<Shared>,
    tx: Mutex<Option<mpsc::Sender<Msg>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl CachedStore {
    /// Loads `backend` on the calling thread, then serves reads from the cache and writes through a worker.
    pub fn open(mut backend: impl ThemeBackend) -> Self {
        let mut state = State::default();
        match backend.load() {
            Ok(snapshot) => state = State::from_snapshot(snapshot),
            Err(message) => state.error = Some(message),
        }
        let shared = Arc::new(Shared {
            cache: Cache::new(state),
            pending: AtomicUsize::new(0),
        });
        let (tx, rx) = mpsc::channel();
        let worker_shared = Arc::clone(&shared);
        let worker = std::thread::Builder::new()
            .name("theme-store".to_owned())
            .spawn(move || run_worker(backend, rx, worker_shared))
            .map_err(|e| log::error!("theme store worker did not start: {e}"))
            .ok();
        Self {
            shared,
            tx: Mutex::new(Some(tx)),
            worker: Mutex::new(worker),
        }
    }

    fn send(&self, msg: Msg) -> bool {
        lock(&self.tx)
            .as_ref()
            .is_some_and(|tx| tx.send(msg).is_ok())
    }

    fn submit(&self, op: StoreOp) {
        for op in self.shared.cache.submit(op) {
            self.shared.pending.fetch_add(1, Ordering::AcqRel);
            if !self.send(Msg::Op(Box::new(op))) {
                self.shared.pending.fetch_sub(1, Ordering::AcqRel);
                self.shared
                    .cache
                    .set_error("the theme store worker is not running".to_owned());
            }
        }
    }

    /// Re-reads the backend in the background and refreshes the cache once its queue is empty.
    pub fn reload(&self) {
        self.send(Msg::Reload);
    }

    /// Blocks until every queued write has reached the backend.
    pub fn flush(&self) {
        let (done_tx, done_rx) = mpsc::channel();
        if self.send(Msg::Flush(done_tx)) {
            let _ = done_rx.recv_timeout(Duration::from_secs(30));
        }
    }
}

impl Drop for CachedStore {
    fn drop(&mut self) {
        self.send(Msg::Shutdown);
        lock(&self.tx).take();
        if let Some(worker) = lock(&self.worker).take() {
            let _ = worker.join();
        }
    }
}

impl ThemeStore for CachedStore {
    fn load_active(&self) -> Option<SavedTheme> {
        lock(&self.shared.cache.state).active.clone()
    }

    fn save_active(&self, theme: &SavedTheme) {
        self.submit(StoreOp::SaveActive(theme.clone()));
    }

    fn clear_active(&self) {
        self.submit(StoreOp::ClearActive);
    }

    fn user_themes(&self) -> Vec<NamedTheme> {
        lock(&self.shared.cache.state).user.clone()
    }

    fn save_user_theme(&self, name: &str, theme: &SavedTheme) {
        self.submit(StoreOp::SaveUser(named(name, theme)));
    }

    fn delete_user_theme(&self, name: &str) {
        self.submit(StoreOp::DeleteUser(name.to_owned()));
    }

    fn rename_user_theme(&self, from: &str, to: &str) {
        self.submit(StoreOp::RenameUser {
            from: from.to_owned(),
            to: to.to_owned(),
        });
    }

    fn startup(&self) -> StartupTheme {
        lock(&self.shared.cache.state).startup.clone()
    }

    fn set_startup(&self, startup: StartupTheme) {
        self.submit(StoreOp::SetStartup(startup));
    }

    fn revision(&self) -> u64 {
        self.shared.cache.revision.load(Ordering::Acquire)
    }

    fn last_error(&self) -> Option<String> {
        lock(&self.shared.cache.state).error.clone()
    }

    fn clear_error(&self) {
        self.shared.cache.clear_error();
    }
}

/// Keeps only the last of any run of ops that overwrite the active theme.
fn coalesce(batch: Vec<StoreOp>) -> Vec<StoreOp> {
    let mut out: Vec<StoreOp> = Vec::with_capacity(batch.len());
    for op in batch {
        if matches!(op, StoreOp::SaveActive(_) | StoreOp::ClearActive) {
            out.retain(|o| !matches!(o, StoreOp::SaveActive(_) | StoreOp::ClearActive));
        }
        out.push(op);
    }
    out
}

fn run_worker(mut backend: impl ThemeBackend, rx: mpsc::Receiver<Msg>, shared: Arc<Shared>) {
    let mut open = true;
    while open {
        let Ok(first) = rx.recv() else {
            break;
        };
        let mut msgs = vec![first];
        msgs.extend(rx.try_iter());

        let mut ops = Vec::new();
        let mut control = Vec::new();
        for msg in msgs {
            match msg {
                Msg::Op(op) => ops.push(*op),
                other => control.push((ops.len(), other)),
            }
        }
        let count = ops.len();
        let ops = coalesce(ops);
        for op in &ops {
            if let Err(message) = backend.apply(op) {
                log::warn!("theme store: {message}");
                shared.cache.set_error(message);
            }
        }
        shared.pending.fetch_sub(count, Ordering::AcqRel);
        for (_, msg) in control {
            match msg {
                Msg::Reload => reload_cache(&mut backend, &shared),
                Msg::Flush(done) => {
                    let _ = done.send(());
                }
                Msg::Shutdown => open = false,
                Msg::Op(_) => {}
            }
        }
    }
}

fn reload_cache(backend: &mut impl ThemeBackend, shared: &Shared) {
    match backend.load() {
        Ok(snapshot) if shared.pending.load(Ordering::Acquire) == 0 => {
            shared.cache.replace(snapshot);
        }
        Ok(_) => {}
        Err(message) => shared.cache.set_error(message),
    }
}

/// Storage key of the user theme list in eframe storage.
pub const USER_THEMES_KEY: &str = "egui_theme_kit.user_themes.v1";
/// Storage key of the startup choice in eframe storage.
pub const STARTUP_KEY: &str = "egui_theme_kit.startup.v1";

#[derive(Serialize, Deserialize)]
struct Envelope {
    name: String,
    #[serde(default)]
    updated_at: u64,
    theme: serde_json::Value,
}

impl Envelope {
    fn from_named(named: &NamedTheme) -> anyhow::Result<Self> {
        Ok(Self {
            name: named.name.clone(),
            updated_at: named.updated_at,
            theme: serde_json::from_slice(&theme_to_json(&named.theme)?)?,
        })
    }

    fn into_named(self) -> anyhow::Result<NamedTheme> {
        let json = serde_json::to_vec(&self.theme)?;
        Ok(NamedTheme {
            name: self.name,
            theme: theme_from_json(&json)?,
            updated_at: self.updated_at,
        })
    }
}

/// A [`ThemeStore`] over eframe's key-value storage: read once at startup, written by [`EframeStorageStore::save_to`].
pub struct EframeStorageStore {
    cache: Cache,
}

impl EframeStorageStore {
    /// Reads the active theme, user themes and startup choice from `storage`; unreadable entries are skipped and reported.
    pub fn from_storage(storage: Option<&dyn eframe::Storage>) -> Self {
        let mut snapshot = StoreSnapshot::default();
        if let Some(storage) = storage {
            snapshot.active = persist::load_stored(Some(storage));
            if let Some(json) = storage.get_string(USER_THEMES_KEY) {
                match serde_json::from_str::<Vec<serde_json::Value>>(&json) {
                    Ok(rows) => {
                        for row in rows {
                            let named = serde_json::from_value::<Envelope>(row)
                                .map_err(anyhow::Error::from)
                                .and_then(Envelope::into_named);
                            match named {
                                Ok(named) => snapshot.user_themes.push(named),
                                Err(e) => snapshot.warnings.push(format!("skipped a theme: {e:#}")),
                            }
                        }
                    }
                    Err(e) => snapshot
                        .warnings
                        .push(format!("saved themes could not be read: {e}")),
                }
            }
            if let Some(json) = storage.get_string(STARTUP_KEY) {
                snapshot.startup = serde_json::from_str(&json).unwrap_or_default();
            }
        }
        Self {
            cache: Cache::new(State::from_snapshot(snapshot)),
        }
    }

    /// Writes the cache to `storage`; the active entry is removed while it equals the default theme.
    pub fn save_to(&self, storage: &mut dyn eframe::Storage) {
        let state = lock(&self.cache.state);
        match &state.active {
            Some(active) if !persist::is_embedded_default(active) => match theme_to_json(active) {
                Ok(json) => storage.set_string(
                    persist::STORAGE_KEY,
                    String::from_utf8_lossy(&json).into_owned(),
                ),
                Err(e) => log::warn!("theme was not saved: {e}"),
            },
            _ => storage.remove_string(persist::STORAGE_KEY),
        }
        let rows: Vec<Envelope> = state
            .user
            .iter()
            .filter_map(|named| Envelope::from_named(named).ok())
            .collect();
        if rows.is_empty() {
            storage.remove_string(USER_THEMES_KEY);
        } else if let Ok(json) = serde_json::to_string(&rows) {
            storage.set_string(USER_THEMES_KEY, json);
        }
        match (&state.startup, serde_json::to_string(&state.startup)) {
            (StartupTheme::LastUsed, _) | (_, Err(_)) => storage.remove_string(STARTUP_KEY),
            (_, Ok(json)) => storage.set_string(STARTUP_KEY, json),
        }
    }
}

impl ThemeStore for EframeStorageStore {
    cache_reads!();

    fn save_active(&self, theme: &SavedTheme) {
        self.cache.submit(StoreOp::SaveActive(theme.clone()));
    }

    fn clear_active(&self) {
        self.cache.submit(StoreOp::ClearActive);
    }

    fn save_user_theme(&self, name: &str, theme: &SavedTheme) {
        self.cache.submit(StoreOp::SaveUser(named(name, theme)));
    }

    fn delete_user_theme(&self, name: &str) {
        self.cache.submit(StoreOp::DeleteUser(name.to_owned()));
    }

    fn rename_user_theme(&self, from: &str, to: &str) {
        self.cache.submit(StoreOp::RenameUser {
            from: from.to_owned(),
            to: to.to_owned(),
        });
    }

    fn set_startup(&self, startup: StartupTheme) {
        self.cache.submit(StoreOp::SetStartup(startup));
    }
}

#[cfg(feature = "files")]
pub use dir::JsonDirStore;

#[cfg(feature = "files")]
mod dir {
    use std::collections::HashMap;
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    const ACTIVE_FILE: &str = "active.json";
    const STARTUP_FILE: &str = "startup.json";

    /// A [`ThemeStore`] keeping `active.json`, `startup.json` and one JSON file per user theme in a directory.
    pub struct JsonDirStore {
        inner: CachedStore,
        dir: PathBuf,
    }

    impl JsonDirStore {
        /// Opens `dir`, creating it; unreadable files are skipped and reported through [`ThemeStore::last_error`].
        pub fn open(dir: impl Into<PathBuf>) -> io::Result<Self> {
            let dir = dir.into();
            fs::create_dir_all(&dir)?;
            let backend = DirBackend::new(dir.clone());
            Ok(Self {
                inner: CachedStore::open(backend),
                dir,
            })
        }

        /// The per-user config directory for `app`, with a `themes` folder inside it.
        pub fn default_dir(app: &str) -> Option<PathBuf> {
            directories::ProjectDirs::from("", "", app).map(|dirs| dirs.config_dir().join("themes"))
        }

        pub fn dir(&self) -> &Path {
            &self.dir
        }

        /// Blocks until every queued write has reached the disk.
        pub fn flush(&self) {
            self.inner.flush();
        }

        /// Re-reads the directory in the background.
        pub fn reload(&self) {
            self.inner.reload();
        }
    }

    macro_rules! delegate {
        ($($name:ident($($arg:ident: $ty:ty),*) $(-> $ret:ty)?;)*) => {
            $(fn $name(&self, $($arg: $ty),*) $(-> $ret)? { self.inner.$name($($arg),*) })*
        };
    }

    impl ThemeStore for JsonDirStore {
        delegate! {
            load_active() -> Option<SavedTheme>;
            save_active(theme: &SavedTheme);
            clear_active();
            user_themes() -> Vec<NamedTheme>;
            save_user_theme(name: &str, theme: &SavedTheme);
            delete_user_theme(name: &str);
            rename_user_theme(from: &str, to: &str);
            startup() -> StartupTheme;
            set_startup(startup: StartupTheme);
            revision() -> u64;
            last_error() -> Option<String>;
            clear_error();
        }
    }

    struct DirBackend {
        dir: PathBuf,
        files: HashMap<String, PathBuf>,
    }

    static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Writes `bytes` to a sibling temporary file, then renames it over `path`.
    fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let tmp = path.with_file_name(format!(".{name}.tmp-{}-{n}", std::process::id()));
        let result = fs::write(&tmp, bytes).and_then(|()| fs::rename(&tmp, path));
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }

    fn remove_if_present(path: &Path) -> io::Result<()> {
        match fs::remove_file(path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    fn fnv1a(text: &str) -> u32 {
        text.bytes().fold(0x811c_9dc5, |h, b| {
            (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
        })
    }

    /// A file name stem for `name`: itself when it is safe, else a sanitized form with a hash suffix.
    pub(super) fn file_stem(name: &str) -> String {
        let safe: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let lower = safe.to_lowercase();
        let reserved = matches!(
            lower.as_str(),
            "active" | "startup" | "con" | "prn" | "aux" | "nul"
        ) || ["com", "lpt"].iter().any(|p| {
            lower
                .strip_prefix(p)
                .is_some_and(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit())
        });
        if safe == name && !reserved && !name.starts_with('.') {
            return safe;
        }
        let stem = safe.trim_matches(|c: char| c == '_' || c == '.' || c == ' ');
        let stem = if stem.is_empty() { "theme" } else { stem };
        format!("{stem}-{:08x}", fnv1a(&name.to_lowercase()))
    }

    impl DirBackend {
        fn new(dir: PathBuf) -> Self {
            Self {
                dir,
                files: HashMap::new(),
            }
        }

        fn path_for(&mut self, name: &str) -> PathBuf {
            let key = name.to_lowercase();
            if let Some(path) = self.files.get(&key) {
                return path.clone();
            }
            let taken: Vec<&PathBuf> = self.files.values().collect();
            let mut stem = file_stem(name);
            let mut path = self.dir.join(format!("{stem}.json"));
            while taken.iter().any(|p| p.as_path() == path) {
                stem = format!("{stem}-{:08x}", fnv1a(name));
                path = self.dir.join(format!("{stem}.json"));
            }
            self.files.insert(key, path.clone());
            path
        }

        fn read_theme_file(path: &Path) -> anyhow::Result<NamedTheme> {
            let bytes = fs::read(path)?;
            let value: serde_json::Value = serde_json::from_slice(&bytes)?;
            if value.get("theme").is_some() && value.get("name").is_some() {
                return serde_json::from_value::<Envelope>(value)?.into_named();
            }
            let stem = path
                .file_stem()
                .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
            let updated_at = fs::metadata(path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            Ok(NamedTheme {
                name: clean_name(&stem)?,
                theme: theme_from_json(&bytes)?,
                updated_at,
            })
        }
    }

    impl ThemeBackend for DirBackend {
        fn load(&mut self) -> Result<StoreSnapshot, String> {
            let mut snapshot = StoreSnapshot::default();
            self.files.clear();
            let active = self.dir.join(ACTIVE_FILE);
            match fs::read(&active) {
                Ok(bytes) => match theme_from_json(&bytes) {
                    Ok(theme) => snapshot.active = Some(theme),
                    Err(e) => snapshot
                        .warnings
                        .push(format!("{ACTIVE_FILE} is unreadable: {e:#}")),
                },
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => snapshot.warnings.push(format!("{ACTIVE_FILE}: {e}")),
            }
            if let Ok(bytes) = fs::read(self.dir.join(STARTUP_FILE)) {
                snapshot.startup = serde_json::from_slice(&bytes).unwrap_or_default();
            }
            let entries =
                fs::read_dir(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
            let mut paths: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
                .filter(|p| {
                    !p.file_name()
                        .is_some_and(|n| n == ACTIVE_FILE || n == STARTUP_FILE)
                })
                .collect();
            paths.sort();
            for path in paths {
                let label = path
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                match Self::read_theme_file(&path) {
                    Ok(named) => {
                        self.files.entry(named.name.to_lowercase()).or_insert(path);
                        snapshot.user_themes.push(named);
                    }
                    Err(e) => snapshot
                        .warnings
                        .push(format!("{label} is unreadable: {e:#}")),
                }
            }
            Ok(snapshot)
        }

        fn apply(&mut self, op: &StoreOp) -> Result<(), String> {
            let fail = |what: &str, e: &dyn std::fmt::Display| format!("{what}: {e}");
            match op {
                StoreOp::SaveActive(theme) => {
                    let json = theme_to_json(theme).map_err(|e| fail(ACTIVE_FILE, &e))?;
                    atomic_write(&self.dir.join(ACTIVE_FILE), &json)
                        .map_err(|e| fail(ACTIVE_FILE, &e))
                }
                StoreOp::ClearActive => remove_if_present(&self.dir.join(ACTIVE_FILE))
                    .map_err(|e| fail(ACTIVE_FILE, &e)),
                StoreOp::SaveUser(named) => {
                    let envelope =
                        Envelope::from_named(named).map_err(|e| fail(&named.name, &e))?;
                    let json =
                        serde_json::to_vec_pretty(&envelope).map_err(|e| fail(&named.name, &e))?;
                    let path = self.path_for(&named.name);
                    atomic_write(&path, &json).map_err(|e| fail(&named.name, &e))
                }
                StoreOp::DeleteUser(name) => {
                    let path = self.files.remove(&name.to_lowercase());
                    match path {
                        Some(path) => remove_if_present(&path).map_err(|e| fail(name, &e)),
                        None => Ok(()),
                    }
                }
                StoreOp::RenameUser { from, to } => {
                    let old = self
                        .files
                        .remove(&from.to_lowercase())
                        .ok_or_else(|| format!("{from}: no such theme file"))?;
                    let mut named = Self::read_theme_file(&old).map_err(|e| fail(from, &e))?;
                    named.name = to.clone();
                    let envelope = Envelope::from_named(&named).map_err(|e| fail(to, &e))?;
                    let json = serde_json::to_vec_pretty(&envelope).map_err(|e| fail(to, &e))?;
                    let path = self.path_for(to);
                    atomic_write(&path, &json).map_err(|e| fail(to, &e))?;
                    if path != old {
                        remove_if_present(&old).map_err(|e| fail(from, &e))?;
                    }
                    Ok(())
                }
                StoreOp::SetStartup(startup) => {
                    let path = self.dir.join(STARTUP_FILE);
                    match startup {
                        StartupTheme::LastUsed => {
                            remove_if_present(&path).map_err(|e| fail(STARTUP_FILE, &e))
                        }
                        other => {
                            let json =
                                serde_json::to_vec(other).map_err(|e| fail(STARTUP_FILE, &e))?;
                            atomic_write(&path, &json).map_err(|e| fail(STARTUP_FILE, &e))
                        }
                    }
                }
            }
        }
    }
}

/// The live theme's identity for change detection: the installed style by pointer, plus the parts kept outside it.
pub(crate) struct Fingerprint {
    style: Arc<Style>,
    glass: GlassParams,
    density: super::metrics::Density,
    text_scale: super::fonts::TextScale,
    semantic: SemanticColors,
}

impl Fingerprint {
    pub(crate) fn of(ctx: &Context) -> Self {
        Self {
            style: ctx.global_style(),
            glass: super::glass::params(ctx),
            density: super::metrics::density(ctx),
            text_scale: super::fonts::text_scale(ctx),
            semantic: SemanticColors::current(ctx),
        }
    }

    pub(crate) fn same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.style, &other.style)
            && self.glass == other.glass
            && self.density == other.density
            && self.text_scale == other.text_scale
            && self.semantic == other.semantic
    }
}

/// Saves the live theme to a [`ThemeStore`] a short while after it last changed.
pub struct AutoSave {
    debounce: Duration,
    seen: Option<Fingerprint>,
    changed_at: Option<Instant>,
}

impl Default for AutoSave {
    fn default() -> Self {
        Self::new(Duration::from_millis(600))
    }
}

impl AutoSave {
    pub fn new(debounce: Duration) -> Self {
        Self {
            debounce,
            seen: None,
            changed_at: None,
        }
    }

    /// Treats the current theme as already stored; call after applying the startup theme.
    pub fn mark_clean(&mut self, ctx: &Context) {
        self.seen = Some(Fingerprint::of(ctx));
        self.changed_at = None;
    }

    /// Whether a change is waiting for its debounce to elapse.
    pub fn is_pending(&self) -> bool {
        self.changed_at.is_some()
    }

    /// Notes changes to the live theme and saves once it has been still for the debounce; call every frame.
    pub fn update(&mut self, ctx: &Context, store: &dyn ThemeStore) {
        let now = Fingerprint::of(ctx);
        match &self.seen {
            None => self.seen = Some(now),
            Some(seen) if !seen.same_as(&now) => {
                self.seen = Some(now);
                self.changed_at = Some(Instant::now());
            }
            Some(_) => {}
        }
        let Some(changed_at) = self.changed_at else {
            return;
        };
        let elapsed = changed_at.elapsed();
        if elapsed >= self.debounce {
            self.persist(ctx, store);
        } else {
            ctx.request_repaint_after(self.debounce - elapsed);
        }
    }

    /// Saves a pending change now; call when closing the app or the picker.
    pub fn flush(&mut self, ctx: &Context, store: &dyn ThemeStore) {
        if self.changed_at.is_some() {
            self.persist(ctx, store);
        }
    }

    fn persist(&mut self, ctx: &Context, store: &dyn ThemeStore) {
        self.changed_at = None;
        if matches!(persist::source(ctx), ThemeSource::File(_)) {
            return;
        }
        let current = SavedTheme::current(ctx);
        if persist::is_embedded_default(&current) {
            store.clear_active();
        } else {
            store.save_active(&current);
        }
    }
}

/// A directory under the system temp dir that is removed on drop.
#[cfg(test)]
pub(crate) struct TestDir(std::path::PathBuf);

#[cfg(test)]
impl TestDir {
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.0
    }
}

#[cfg(test)]
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
pub(crate) fn test_dir(tag: &str) -> TestDir {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("egui_theme_kit_{tag}_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create the test directory");
    TestDir(dir)
}

#[cfg(test)]
mod tests {
    use super::super::{
        metrics::Density,
        presets::{PresetStyles, apply_preset, apply_saved_theme, installed_preset, set_density},
        theme_chrome::graphite_theme,
    };
    use super::*;
    use eframe::Storage as _;
    use std::collections::HashMap;

    fn preset_theme(preset: PresetStyles) -> SavedTheme {
        let ctx = Context::default();
        apply_preset(&ctx, preset);
        SavedTheme::current(&ctx)
    }

    fn recognised(theme: &SavedTheme) -> Option<&'static str> {
        presets::exact_preset(&presets::normalized(&theme.style)).map(|d| d.id)
    }

    #[derive(Default)]
    struct MemStorage(HashMap<String, String>);

    impl eframe::Storage for MemStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }

        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }

        fn flush(&mut self) {}
    }

    /// The contract every store implementation must meet.
    fn exercise_store(store: &dyn ThemeStore) {
        let nebula = preset_theme(PresetStyles::Nebula);
        let noir = preset_theme(PresetStyles::Noir);
        let start = store.revision();

        assert!(store.load_active().is_none());
        store.save_active(&nebula);
        assert_eq!(recognised(&store.load_active().unwrap()), Some("nebula"));
        assert!(store.revision() > start);

        store.save_user_theme("  Late   Night ", &noir);
        store.save_user_theme("alpha", &nebula);
        let names: Vec<String> = store.user_themes().into_iter().map(|t| t.name).collect();
        assert_eq!(names, ["alpha", "Late Night"]);
        assert!(store.user_themes().iter().all(|t| t.updated_at > 0));

        store.save_user_theme("LATE NIGHT", &nebula);
        let late = store.user_themes();
        assert_eq!(late.len(), 2, "same name in another case replaces");
        assert_eq!(recognised(&late[1].theme), Some("nebula"));

        let before = store.revision();
        store.rename_user_theme("alpha", "Zeta");
        assert!(store.revision() > before);
        let names: Vec<String> = store.user_themes().into_iter().map(|t| t.name).collect();
        assert_eq!(names, ["LATE NIGHT", "Zeta"]);

        store.rename_user_theme("Zeta", "late night");
        assert!(store.last_error().unwrap().contains("already exists"));
        assert_eq!(store.user_themes().len(), 2);
        store.clear_error();
        assert!(store.last_error().is_none());

        store.rename_user_theme("missing", "other");
        assert!(store.last_error().unwrap().contains("no theme named"));
        store.clear_error();

        store.save_user_theme("   ", &noir);
        assert!(store.last_error().is_some());
        store.clear_error();
        assert_eq!(store.user_themes().len(), 2);

        assert_eq!(store.startup(), StartupTheme::LastUsed);
        store.set_startup(StartupTheme::User("zeta".to_owned()));
        assert_eq!(store.startup(), StartupTheme::User("Zeta".to_owned()));
        store.rename_user_theme("Zeta", "Omega");
        assert_eq!(store.startup(), StartupTheme::User("Omega".to_owned()));
        store.delete_user_theme("omega");
        assert_eq!(store.startup(), StartupTheme::LastUsed);
        store.set_startup(StartupTheme::Preset("Nebula Glass".to_owned()));
        assert_eq!(store.startup(), StartupTheme::Preset("nebula".to_owned()));
        store.set_startup(StartupTheme::Preset("nope".to_owned()));
        assert!(store.last_error().is_some());
        store.clear_error();

        store.clear_active();
        assert!(store.load_active().is_none());
        let names: Vec<String> = store.user_themes().into_iter().map(|t| t.name).collect();
        assert_eq!(names, ["LATE NIGHT"]);
    }

    #[test]
    fn names_are_cleaned_and_validated() {
        assert_eq!(clean_name("  a \t b  ").as_deref(), Ok("a b"));
        assert_eq!(clean_name(" \n "), Err(NameError::Empty));
        assert_eq!(clean_name(&"x".repeat(65)), Err(NameError::TooLong));
        assert_eq!(clean_name(&"x".repeat(64)).map(|n| n.len()), Ok(64));
        assert_eq!(clean_name("a\u{7}b"), Err(NameError::Control));
        assert!(NameError::Empty.to_string().contains("name"));
    }

    #[test]
    fn the_memory_store_meets_the_contract() {
        exercise_store(&MemoryStore::default());
    }

    #[test]
    fn the_eframe_store_meets_the_contract() {
        exercise_store(&EframeStorageStore::from_storage(None));
    }

    #[cfg(feature = "files")]
    #[test]
    fn the_directory_store_meets_the_contract_and_survives_a_restart() {
        let dir = test_dir("store_contract");
        let store = JsonDirStore::open(dir.path()).unwrap();
        exercise_store(&store);
        store.flush();
        assert!(store.last_error().is_none());
    }

    #[cfg(feature = "files")]
    #[test]
    fn the_directory_store_persists_every_kind_of_record() {
        let dir = test_dir("store_roundtrip");
        {
            let store = JsonDirStore::open(dir.path()).unwrap();
            store.save_active(&preset_theme(PresetStyles::Aurora));
            store.save_user_theme("Day", &preset_theme(PresetStyles::Velvet));
            store.save_user_theme("Night/Owl: 2", &preset_theme(PresetStyles::Galactic));
            store.set_startup(StartupTheme::User("Day".to_owned()));
        }
        let files: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(files.iter().all(|f| !f.contains(".tmp")), "{files:?}");
        assert!(files.contains(&"active.json".to_owned()));
        assert!(files.contains(&"Day.json".to_owned()));
        assert!(files.iter().all(|f| !f.contains('/') && !f.contains(':')));

        let store = JsonDirStore::open(dir.path()).unwrap();
        assert_eq!(recognised(&store.load_active().unwrap()), Some("aurora"));
        let themes = store.user_themes();
        let names: Vec<&str> = themes.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Day", "Night/Owl: 2"]);
        assert_eq!(recognised(&themes[1].theme), Some("galactic"));
        assert_eq!(store.startup(), StartupTheme::User("Day".to_owned()));
        assert!(store.last_error().is_none());

        store.rename_user_theme("Night/Owl: 2", "Owl");
        store.delete_user_theme("Day");
        store.set_startup(StartupTheme::LastUsed);
        store.clear_active();
        drop(store);
        let reopened = JsonDirStore::open(dir.path()).unwrap();
        let names: Vec<String> = reopened.user_themes().into_iter().map(|t| t.name).collect();
        assert_eq!(names, ["Owl"]);
        assert!(reopened.load_active().is_none());
        assert_eq!(reopened.startup(), StartupTheme::LastUsed);
        assert!(!dir.path().join("active.json").exists());
        assert!(!dir.path().join("startup.json").exists());
        assert!(!dir.path().join("Day.json").exists());
    }

    #[cfg(feature = "files")]
    #[test]
    fn corrupt_files_are_skipped_reported_and_left_in_place() {
        let dir = test_dir("store_corrupt");
        let good = preset_theme(PresetStyles::Quartz);
        {
            let store = JsonDirStore::open(dir.path()).unwrap();
            store.save_user_theme("Good", &good);
        }
        std::fs::write(dir.path().join("broken.json"), b"{ not json").unwrap();
        std::fs::write(dir.path().join("empty.json"), b"").unwrap();
        std::fs::write(dir.path().join("active.json"), b"[1, 2").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"ignored").unwrap();

        let store = JsonDirStore::open(dir.path()).unwrap();
        let names: Vec<String> = store.user_themes().into_iter().map(|t| t.name).collect();
        assert_eq!(names, ["Good"]);
        assert!(store.load_active().is_none());
        let error = store.last_error().unwrap();
        assert!(
            error.contains("broken.json") && error.contains("active.json"),
            "{error}"
        );
        assert!(dir.path().join("broken.json").exists());
        store.clear_error();
        assert!(store.last_error().is_none());

        store.save_user_theme("broken", &good);
        store.flush();
        assert_eq!(store.user_themes().len(), 2);
        assert!(store.last_error().is_none());
    }

    #[cfg(feature = "files")]
    #[test]
    fn a_bare_theme_file_is_listed_under_its_file_name() {
        let dir = test_dir("store_bare");
        let theme = preset_theme(PresetStyles::TokyoNight);
        std::fs::write(
            dir.path().join("My Import.json"),
            theme_to_json(&theme).unwrap(),
        )
        .unwrap();
        let store = JsonDirStore::open(dir.path()).unwrap();
        let themes = store.user_themes();
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].name, "My Import");
        assert_eq!(recognised(&themes[0].theme), Some("tokyo-night"));
    }

    #[cfg(feature = "files")]
    #[test]
    fn file_stems_never_collide_for_different_names_or_reserved_words() {
        use super::dir::file_stem;
        assert_eq!(file_stem("Plain Name-1_a"), "Plain Name-1_a");
        assert_ne!(file_stem("a/b"), file_stem("a:b"));
        assert_ne!(file_stem("active"), "active");
        assert_ne!(file_stem("Startup"), "Startup");
        for reserved in ["CON", "nul", "Com1", "LPT9"] {
            assert_ne!(file_stem(reserved), reserved, "{reserved}");
        }
        assert_eq!(file_stem("Console"), "Console");
        assert_eq!(file_stem("com10"), "com10");
        assert!(file_stem("???").starts_with("theme-"));
        assert!(!file_stem(".hidden").starts_with('.'));
    }

    #[test]
    fn the_cached_store_writes_in_order_through_a_backend_and_flushes_on_drop() {
        #[derive(Clone, Default)]
        struct Log(Arc<Mutex<Vec<String>>>);

        struct Backend {
            log: Log,
            fail_on: Option<&'static str>,
        }

        impl ThemeBackend for Backend {
            fn load(&mut self) -> Result<StoreSnapshot, String> {
                Ok(StoreSnapshot {
                    user_themes: vec![named("Seed", &graphite_theme())],
                    ..Default::default()
                })
            }

            fn apply(&mut self, op: &StoreOp) -> Result<(), String> {
                let label = match op {
                    StoreOp::SaveActive(_) => "save_active".to_owned(),
                    StoreOp::ClearActive => "clear_active".to_owned(),
                    StoreOp::SaveUser(n) => format!("save_user:{}", n.name),
                    StoreOp::DeleteUser(n) => format!("delete_user:{n}"),
                    StoreOp::RenameUser { from, to } => format!("rename:{from}>{to}"),
                    StoreOp::SetStartup(_) => "startup".to_owned(),
                };
                if self.fail_on == Some("save_user:Bad") && label == "save_user:Bad" {
                    return Err("disk full".to_owned());
                }
                self.log.0.lock().unwrap().push(label);
                Ok(())
            }
        }

        let log = Log::default();
        let store = CachedStore::open(Backend {
            log: log.clone(),
            fail_on: Some("save_user:Bad"),
        });
        assert_eq!(store.user_themes().len(), 1);
        store.save_user_theme("Mine", &graphite_theme());
        store.rename_user_theme("Mine", "Yours");
        store.save_user_theme("Bad", &graphite_theme());
        store.flush();
        assert_eq!(store.last_error().as_deref(), Some("disk full"));
        assert_eq!(
            store.user_themes().len(),
            3,
            "the cache keeps the optimistic state"
        );
        store.delete_user_theme("Seed");
        drop(store);
        assert_eq!(
            *log.0.lock().unwrap(),
            ["save_user:Mine", "rename:Mine>Yours", "delete_user:Seed"]
        );
    }

    #[test]
    fn the_worker_coalesces_runs_of_active_saves() {
        let ops = vec![
            StoreOp::SaveActive(graphite_theme()),
            StoreOp::DeleteUser("x".to_owned()),
            StoreOp::SaveActive(graphite_theme()),
            StoreOp::ClearActive,
            StoreOp::SaveActive(graphite_theme()),
        ];
        let kept = coalesce(ops);
        assert_eq!(kept.len(), 2);
        assert!(matches!(kept[0], StoreOp::DeleteUser(_)));
        assert!(matches!(kept[1], StoreOp::SaveActive(_)));
    }

    #[test]
    fn a_reload_picks_up_backend_changes_made_elsewhere() {
        struct Shared(Arc<Mutex<Vec<NamedTheme>>>);

        impl ThemeBackend for Shared {
            fn load(&mut self) -> Result<StoreSnapshot, String> {
                Ok(StoreSnapshot {
                    user_themes: self.0.lock().unwrap().clone(),
                    ..Default::default()
                })
            }

            fn apply(&mut self, _: &StoreOp) -> Result<(), String> {
                Ok(())
            }
        }

        let rows = Arc::new(Mutex::new(Vec::new()));
        let store = CachedStore::open(Shared(Arc::clone(&rows)));
        assert!(store.user_themes().is_empty());
        let before = store.revision();
        rows.lock()
            .unwrap()
            .push(named("Elsewhere", &graphite_theme()));
        store.reload();
        store.flush();
        assert_eq!(store.user_themes().len(), 1);
        assert!(store.revision() > before);
    }

    #[test]
    fn a_failing_load_is_reported_and_leaves_an_empty_store() {
        struct Broken;

        impl ThemeBackend for Broken {
            fn load(&mut self) -> Result<StoreSnapshot, String> {
                Err("database unreachable".to_owned())
            }

            fn apply(&mut self, _: &StoreOp) -> Result<(), String> {
                Ok(())
            }
        }

        let store = CachedStore::open(Broken);
        assert_eq!(store.last_error().as_deref(), Some("database unreachable"));
        assert!(store.user_themes().is_empty());
        store.save_user_theme("Still works", &graphite_theme());
        assert_eq!(store.user_themes().len(), 1);
    }

    #[test]
    fn the_eframe_store_round_trips_through_storage_and_removes_the_default_entry() {
        let mut storage = MemStorage::default();
        let store = EframeStorageStore::from_storage(Some(&storage));
        store.save_active(&preset_theme(PresetStyles::Supernova));
        store.save_user_theme("Mine", &preset_theme(PresetStyles::Rerun));
        store.set_startup(StartupTheme::Preset("noir".to_owned()));
        store.save_to(&mut storage);
        assert!(storage.get_string(persist::STORAGE_KEY).is_some());
        assert!(storage.get_string(USER_THEMES_KEY).is_some());
        assert!(storage.get_string(STARTUP_KEY).is_some());

        let again = EframeStorageStore::from_storage(Some(&storage));
        assert_eq!(recognised(&again.load_active().unwrap()), Some("supernova"));
        assert_eq!(again.user_themes()[0].name, "Mine");
        assert_eq!(again.startup(), StartupTheme::Preset("noir".to_owned()));

        again.save_active(&graphite_theme());
        again.delete_user_theme("Mine");
        again.set_startup(StartupTheme::LastUsed);
        again.save_to(&mut storage);
        assert!(storage.get_string(persist::STORAGE_KEY).is_none());
        assert!(storage.get_string(USER_THEMES_KEY).is_none());
        assert!(storage.get_string(STARTUP_KEY).is_none());
    }

    #[test]
    fn the_eframe_store_reads_what_persist_saved() {
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Twilight);
        let mut storage = MemStorage::default();
        persist::save(&ctx, &mut storage);
        let store = EframeStorageStore::from_storage(Some(&storage));
        assert_eq!(recognised(&store.load_active().unwrap()), Some("twilight"));
    }

    #[test]
    fn the_eframe_store_skips_unreadable_records_and_says_so() {
        let mut storage = MemStorage::default();
        storage.set_string(USER_THEMES_KEY, "[{\"name\": 4}, 17]".to_owned());
        storage.set_string(STARTUP_KEY, "garbage".to_owned());
        let store = EframeStorageStore::from_storage(Some(&storage));
        assert!(store.user_themes().is_empty());
        assert_eq!(store.startup(), StartupTheme::LastUsed);
        assert!(store.last_error().unwrap().contains("skipped a theme"));

        storage.set_string(USER_THEMES_KEY, "nonsense".to_owned());
        let store = EframeStorageStore::from_storage(Some(&storage));
        assert!(store.last_error().unwrap().contains("could not be read"));
    }

    fn changed_ctx() -> Context {
        let ctx = Context::default();
        apply_saved_theme(&ctx, graphite_theme());
        ctx
    }

    #[test]
    fn autosave_ignores_the_startup_theme_and_saves_changes_after_the_debounce() {
        let ctx = changed_ctx();
        let store = MemoryStore::default();
        let mut auto = AutoSave::new(Duration::ZERO);
        auto.update(&ctx, &store);
        auto.update(&ctx, &store);
        assert!(
            store.load_active().is_none(),
            "the startup theme is not re-saved"
        );
        assert!(!auto.is_pending());

        apply_preset(&ctx, PresetStyles::Nebula);
        auto.update(&ctx, &store);
        assert_eq!(recognised(&store.load_active().unwrap()), Some("nebula"));

        set_density(&ctx, Density::Comfortable);
        auto.update(&ctx, &store);
        assert_eq!(store.load_active().unwrap().density, Density::Comfortable);
    }

    #[test]
    fn autosave_waits_for_the_debounce_and_flushes_on_demand() {
        let ctx = changed_ctx();
        let store = MemoryStore::default();
        let mut auto = AutoSave::new(Duration::from_secs(3600));
        auto.update(&ctx, &store);
        apply_preset(&ctx, PresetStyles::Aurora);
        auto.update(&ctx, &store);
        assert!(auto.is_pending());
        assert!(store.load_active().is_none());
        auto.flush(&ctx, &store);
        assert!(!auto.is_pending());
        assert_eq!(recognised(&store.load_active().unwrap()), Some("aurora"));
    }

    #[test]
    fn autosave_clears_the_entry_when_the_theme_returns_to_the_default() {
        let ctx = changed_ctx();
        let store = MemoryStore::default();
        let mut auto = AutoSave::new(Duration::ZERO);
        auto.update(&ctx, &store);
        apply_preset(&ctx, PresetStyles::Noir);
        auto.update(&ctx, &store);
        assert!(store.load_active().is_some());
        apply_saved_theme(&ctx, graphite_theme());
        auto.update(&ctx, &store);
        assert!(store.load_active().is_none());
    }

    #[test]
    fn autosave_never_writes_a_theme_file_run_back() {
        let ctx = changed_ctx();
        persist::set_source(&ctx, ThemeSource::File("theme.json".into()));
        let store = MemoryStore::default();
        let mut auto = AutoSave::new(Duration::ZERO);
        auto.update(&ctx, &store);
        apply_preset(&ctx, PresetStyles::Velvet);
        auto.update(&ctx, &store);
        auto.flush(&ctx, &store);
        assert!(store.load_active().is_none());
        assert_eq!(installed_preset(&ctx), PresetStyles::Velvet);
    }

    #[test]
    fn marking_clean_adopts_the_current_theme_without_saving() {
        let ctx = changed_ctx();
        let store = MemoryStore::default();
        let mut auto = AutoSave::new(Duration::ZERO);
        apply_preset(&ctx, PresetStyles::Obsidian);
        auto.mark_clean(&ctx);
        auto.update(&ctx, &store);
        assert!(store.load_active().is_none());
    }

    #[test]
    fn startup_choices_serialize_to_stable_json() {
        for (value, json) in [
            (StartupTheme::LastUsed, r#"{"kind":"last_used"}"#),
            (
                StartupTheme::User("a".into()),
                r#"{"kind":"user","name":"a"}"#,
            ),
            (
                StartupTheme::Preset("noir".into()),
                r#"{"kind":"preset","name":"noir"}"#,
            ),
        ] {
            assert_eq!(serde_json::to_string(&value).unwrap(), json);
            assert_eq!(serde_json::from_str::<StartupTheme>(json).unwrap(), value);
        }
    }
}
