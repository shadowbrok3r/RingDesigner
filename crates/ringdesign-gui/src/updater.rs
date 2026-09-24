//! Background GitHub updates. Downloads are inert until a verified, explicit restart.
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    sync::mpsc,
    time::{Duration, Instant},
};
use tempfile::NamedTempFile;

pub const RELEASES: &str = "https://github.com/shadowbrok3r/RingDesigner/releases";
const API: &str = "https://api.github.com/repos/shadowbrok3r/RingDesigner/releases?per_page=100";
const MAX_BINARY: u64 = 512 * 1024 * 1024;
pub static RESTART: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Said beside Install when the running build carries OpenCascade.
pub const OCCT_WARNING: &str = "This build carries OpenCascade and the update may not: after it, Fillet, Shell and the junction stay greyed and STEP files read only what RingDesigner reads itself, until an occt-worker stands beside the app.";

#[derive(Clone, Debug, Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Clone, Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}
struct Ready {
    file: NamedTempFile,
    version: String,
    size: u64,
    hash: String,
}
enum Event {
    Progress(String),
    Ready(Ready),
    Done(String),
    Failed(String),
    Installed,
}

pub struct Updater {
    pub automatic: bool,
    /// Whether the running build carries the OpenCascade worker.
    pub carries_occt: bool,
    pub status: String,
    pub restart: bool,
    rx: Option<mpsc::Receiver<Event>>,
    ready: Option<Ready>,
    last_check: Option<Instant>,
}
impl Updater {
    pub fn new(automatic: bool) -> Self {
        Self {
            automatic,
            carries_occt: crate::occt_embedded::EMBEDDED.is_present(),
            status: "Updates have not been checked".into(),
            restart: false,
            rx: None,
            ready: None,
            last_check: None,
        }
    }
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }
    pub fn poll(&mut self, ctx: &egui::Context) {
        if self.automatic
            && !self.busy()
            && self.ready.is_none()
            && self
                .last_check
                .is_none_or(|t| t.elapsed() > Duration::from_secs(6 * 3600))
        {
            self.check(ctx);
        }
        let mut events = Vec::new();
        let mut disconnected = false;
        if let Some(rx) = &self.rx {
            loop {
                match rx.try_recv() {
                    Ok(event) => events.push(event),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }
        for event in events {
            match event {
                Event::Progress(s) => self.status = s,
                Event::Ready(r) => {
                    self.status = format!("{} is ready to install", r.version);
                    self.ready = Some(r);
                    self.rx = None;
                }
                Event::Done(s) | Event::Failed(s) => {
                    self.status = s;
                    self.rx = None;
                }
                Event::Installed => {
                    self.rx = None;
                    self.restart = true;
                }
            }
        }
        if disconnected && self.rx.is_some() {
            self.rx = None;
            self.status = "Update worker stopped unexpectedly — Check now to retry".into();
        }
        if self.busy() {
            ctx.request_repaint_after(Duration::from_millis(150));
        }
    }
    pub fn check(&mut self, ctx: &egui::Context) {
        if self.busy() {
            return;
        }
        self.ready = None;
        self.last_check = Some(Instant::now());
        self.status = "Checking GitHub releases…".into();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let event = match download(&tx) {
                Ok(Some(r)) => Event::Ready(r),
                Ok(None) => Event::Done("No newer compatible desktop release".into()),
                Err(e) => Event::Failed(format!("Update failed: {e:#}")),
            };
            let _ = tx.send(event);
            ctx.request_repaint();
        });
    }
    /// Returns an install request; the caller must flush the session first.
    pub fn menu(&mut self, ui: &mut egui::Ui, can_restart: bool) -> bool {
        use ringdesign_workbench::icons::Icon;
        let mut install = false;
        let label = if self.ready.is_some() {
            "Update ready"
        } else {
            "Updates"
        };
        ui.menu_button((Icon::Check.image(ui, 18.), label), |ui| {
            ui.set_max_width(310.);
            ui.label(format!("RingDesigner {}", env!("CARGO_PKG_VERSION")));
            ui.label(&self.status);
            if self.busy() {
                ui.spinner();
            }
            ui.checkbox(
                &mut self.automatic,
                "Automatically check and download updates",
            );
            if ui
                .add_enabled(!self.busy(), egui::Button::new("Check now"))
                .on_disabled_hover_text("An update operation is running")
                .clicked()
            {
                self.check(ui.ctx());
            }
            if self.ready.is_some() {
                install = ui
                    .add_enabled(
                        can_restart && !self.busy(),
                        egui::Button::new("Install and restart"),
                    )
                    .on_disabled_hover_text("Wait for the current build or export to finish")
                    .clicked();
                ui.weak("Your design and workspace will be saved before restarting.");
                if self.carries_occt {
                    ui.colored_label(crate::theme::WARN, OCCT_WARNING);
                }
            }
            ui.hyperlink_to("Release notes", RELEASES);
        });
        install
    }
    pub fn install(&mut self, ctx: &egui::Context) {
        let Some(ready) = self.ready.take() else {
            return;
        };
        self.status = "Installing verified update…".into();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<()> {
                // Verify again just before replacing the running executable.
                verify(
                    std::fs::File::open(ready.file.path())?,
                    ready.size,
                    &ready.hash,
                    std::io::sink(),
                    |_| {},
                )?;
                verify_native(ready.file.path())?;
                self_replace::self_replace(ready.file.path())
                    .context("Cannot replace the executable; install in a writable folder")?;
                Ok(())
            })();
            let _ = tx.send(match result {
                Ok(()) => Event::Installed,
                Err(e) => Event::Failed(format!("Install failed: {e:#}")),
            });
            ctx.request_repaint();
        });
    }
}

fn asset_name(os: &str, arch: &str) -> Option<String> {
    let target = match (os, arch) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        _ => return None,
    };
    Some(format!(
        "ringdesigner-{target}{}",
        if os == "windows" { ".exe" } else { "" }
    ))
}
fn version(tag: &str) -> Option<semver::Version> {
    semver::Version::parse(tag.strip_prefix("desktop-v")?)
        .ok()
        .filter(|v| v.pre.is_empty())
}
fn choose<'a>(
    releases: &'a [Release],
    current: &semver::Version,
    name: &str,
) -> Option<(&'a Release, &'a Asset)> {
    releases
        .iter()
        .filter(|r| !r.draft && !r.prerelease)
        .filter_map(|r| Some((version(&r.tag_name)?, r)))
        .filter(|(v, _)| v > current)
        .filter_map(|(v, r)| Some((v, r, r.assets.iter().find(|a| a.name == name)?)))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, r, a)| (r, a))
}
fn trusted_asset(url: &str) -> bool {
    url.starts_with("https://github.com/shadowbrok3r/RingDesigner/releases/download/")
        && !url.contains(['?', '#'])
}
fn digest(value: &str) -> Result<String> {
    let hash = value.strip_prefix("sha256:").unwrap_or(value);
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("Missing or invalid SHA-256 checksum");
    }
    Ok(hash.to_ascii_lowercase())
}
fn download(tx: &mpsc::Sender<Event>) -> Result<Option<Ready>> {
    let name = asset_name(std::env::consts::OS, std::env::consts::ARCH)
        .context("No automatic update package for this platform")?;
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("RingDesigner/", env!("CARGO_PKG_VERSION")))
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .build()?;
    let response = client
        .get(API)
        .timeout(Duration::from_secs(30))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2026-03-10")
        .send()?
        .error_for_status()?;
    let mut json = Vec::new();
    response.take(2 * 1024 * 1024 + 1).read_to_end(&mut json)?;
    if json.len() > 2 * 1024 * 1024 {
        bail!("Release metadata is too large");
    }
    let releases: Vec<Release> = serde_json::from_slice(&json)?;
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;
    let Some((release, asset)) = choose(&releases, &current, &name) else {
        return Ok(None);
    };
    if !(1..=MAX_BINARY).contains(&asset.size) || !trusted_asset(&asset.browser_download_url) {
        bail!("Invalid release asset");
    }
    let hash = if let Some(hash) = &asset.digest {
        digest(hash)?
    } else {
        let checksum = release
            .assets
            .iter()
            .find(|a| a.name == format!("{name}.sha256"))
            .context("Release has no SHA-256 checksum")?;
        if !trusted_asset(&checksum.browser_download_url) || checksum.size > 512 {
            bail!("Invalid checksum asset");
        }
        let mut text = String::new();
        client
            .get(&checksum.browser_download_url)
            .send()?
            .error_for_status()?
            .take(513)
            .read_to_string(&mut text)?;
        if text.len() > 512 {
            bail!("Checksum is too large");
        }
        digest(text.split_whitespace().next().unwrap_or(""))?
    };
    let mut file = tempfile::Builder::new()
        .prefix("ringdesigner-update-")
        .tempfile()?;
    let _ = tx.send(Event::Progress(format!(
        "Downloading {}…",
        release.tag_name
    )));
    let response = client
        .get(&asset.browser_download_url)
        .send()?
        .error_for_status()?;
    verify(response, asset.size, &hash, &mut file, |n| {
        let _ = tx.send(Event::Progress(format!(
            "Downloading {} — {}%",
            release.tag_name,
            n * 100 / asset.size
        )));
    })?;
    file.as_file().sync_all()?;
    verify_native(file.path())?;
    Ok(Some(Ready {
        file,
        version: release.tag_name.clone(),
        size: asset.size,
        hash,
    }))
}
fn verify_native(path: &std::path::Path) -> Result<()> {
    let mut magic = [0; 4];
    std::fs::File::open(path)?.read_exact(&mut magic)?;
    let valid = match std::env::consts::OS {
        "linux" => magic == *b"\x7fELF",
        "windows" => magic[..2] == *b"MZ",
        "macos" => matches!(magic, [0xcf, 0xfa, 0xed, 0xfe] | [0xfe, 0xed, 0xfa, 0xcf]),
        _ => false,
    };
    if !valid {
        bail!("The release asset is not a native executable");
    }
    Ok(())
}

fn verify(
    mut reader: impl Read,
    size: u64,
    expected: &str,
    mut writer: impl Write,
    mut progress: impl FnMut(u64),
) -> Result<()> {
    if size == 0 || size > MAX_BINARY {
        bail!("Invalid update size");
    }
    let expected = digest(expected)?;
    let mut hash = Sha256::new();
    let mut count = 0;
    let mut buffer = [0; 64 * 1024];
    let mut last = Instant::now();
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > size {
            bail!("Update exceeds its declared size");
        }
        writer.write_all(&buffer[..n])?;
        hash.update(&buffer[..n]);
        if last.elapsed() > Duration::from_millis(200) {
            progress(count);
            last = Instant::now();
        }
    }
    if count != size {
        bail!("Incomplete download: received {count} of {size} bytes");
    }
    if format!("{:x}", hash.finalize()) != expected {
        bail!("SHA-256 verification failed");
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str, name: &str) -> Release {
        Release {
            tag_name: tag.into(),
            draft: false,
            prerelease: false,
            assets: vec![Asset {
                name: name.into(),
                browser_download_url: String::new(),
                size: 3,
                digest: None,
            }],
        }
    }
    #[test]
    fn an_update_offered_to_a_build_carrying_opencascade_warns_it_may_not() {
        use egui_kittest::kittest::Queryable;
        for carries in [true, false] {
            let mut updater = Updater::new(false);
            updater.carries_occt = carries;
            updater.ready = Some(Ready { file: NamedTempFile::new().unwrap(), version: "desktop-v9.9.9".into(), size: 0, hash: String::new() });
            let mut h = egui_kittest::Harness::new_ui_state(|ui, u: &mut Updater| {
                u.menu(ui, true);
            }, updater);
            h.get_by_label("Update ready").click();
            h.run();
            assert!(h.query_by_label("Install and restart").is_some());
            assert_eq!(h.query_by_label(OCCT_WARNING).is_some(), carries);
        }
    }
    #[test]
    fn a_stopped_worker_releases_controls_for_retry() {
        let mut updater = Updater::new(false);
        let (tx, rx) = mpsc::channel();
        updater.rx = Some(rx);
        drop(tx);
        updater.poll(&egui::Context::default());
        assert!(!updater.busy());
        assert!(updater.status.contains("retry"));
    }
    #[test]
    fn only_newer_stable_compatible_desktop_releases() {
        let mut items = vec![
            release("desktop-v0.2.0", "linux"),
            release("desktop-v0.3.0", "windows"),
            release("android-v9.0.0", "linux"),
            release("desktop-v0.4.0-beta.1", "linux"),
            release("desktop-v0.1.0", "linux"),
        ];
        let current = semver::Version::new(0, 1, 0);
        assert_eq!(
            choose(&items, &current, "linux").unwrap().0.tag_name,
            "desktop-v0.2.0"
        );
        items[0].draft = true;
        assert!(choose(&items, &current, "linux").is_none());
        items[0].draft = false;
        items[0].prerelease = true;
        assert!(choose(&items, &current, "linux").is_none());
        assert!(choose(&[], &current, "linux").is_none());
    }
    #[test]
    fn downloads_must_be_complete_bounded_and_unmodified() {
        let hash = format!("{:x}", Sha256::digest(b"abc"));
        let mut out = Vec::new();
        verify(&b"abc"[..], 3, &hash, &mut out, |_| {}).unwrap();
        assert_eq!(out, b"abc");
        assert!(verify(&b"ab"[..], 3, &hash, std::io::sink(), |_| {}).is_err());
        assert!(verify(&b"abcd"[..], 3, &hash, std::io::sink(), |_| {}).is_err());
        assert!(verify(&b"abd"[..], 3, &hash, std::io::sink(), |_| {}).is_err());
        assert!(verify(&b"abc"[..], MAX_BINARY + 1, &hash, std::io::sink(), |_| {}).is_err());
        assert!(digest("sha256:123").is_err());
    }
    #[test]
    fn exact_platform_and_release_origin() {
        assert_eq!(
            asset_name("windows", "x86_64").unwrap(),
            "ringdesigner-x86_64-pc-windows-msvc.exe"
        );
        assert!(asset_name("linux", "aarch64").is_none());
        assert!(!trusted_asset("https://example.com/ringdesigner"));
        assert!(!trusted_asset(
            "https://github.com.evil.test/shadowbrok3r/RingDesigner/releases/download/test"
        ));
    }
}
