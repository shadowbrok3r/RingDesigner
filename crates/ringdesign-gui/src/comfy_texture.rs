//! Seamless tileable height maps from the ComfyUI rig, through comfy-gate.
//!
//! Two calls, both blocking, both meant for a worker thread:
//!
//! ```ignore
//! let gate = Gate::installed()?;
//! let prompt = gate.write_prompt("a snake scale texture")?; // local Qwen writes it
//! let out = gate.render(&Job::new(Preset::TileSquare, prompt))?;
//! let alpha = ringdesign_core::alpha::Alpha::from_png16("snake scales", &out.height_png)?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! `height_png` is the alpha: 16-bit, true height (depth model for the form,
//! the render's own detail on top), seam removed. `relief_png` is the lit
//! luminance and `color_png` the raw render — both are for looking at, not for
//! displacing geometry.
//!
//! The graph is not built here. Two API-format workflows are embedded verbatim
//! from `workflows/Texture` on the rig, and only a handful of widgets are
//! patched: the prompt, the seed, the sampler settings, the relief LoRA
//! strength, the output size, and the save prefixes. Everything the tiling
//! depends on — circular padding on the UNet and the VAE, the plain (never
//! tiled) VAE decode, the tensor-native chain to a 16-bit save — is left
//! exactly as the rig generated it. To change any of that, regenerate the
//! workflows there and re-export the two JSON files beside this module.
//!
//! Auth is one API key belonging to a gate user that owns nothing else, so a
//! render here cannot read anything else on that server.

// Wired into the UI a piece at a time; the unused half is not a defect.
#![allow(dead_code)]

use std::time::{Duration, Instant};
use std::sync::mpsc::{Receiver, Sender};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

/// Trigger word of the BAS-RELIEF LoRA the workflows load. The prompt writer
/// does not emit it (it writes the motif, not the wiring), so it is prepended
/// at submit time; without it the LoRA is loaded but barely steers.
const TRIGGER: &str = "BAS-RELIEF page, ";

/// Default gate. Reachable over the tailnet and from anywhere via Cloudflare.
const DEFAULT_BASE: &str = "https://comfy.shadowbroker.app";

/// Compiled in by `build.rs` from a `.env` or the build environment. Empty
/// means this build has no texture server, which is every CI build — the same
/// "empty string disables the feature" rule the Mastertech `database` crate's
/// injected keys use.
const BAKED_URL: &str = env!("COMFY_GATE_URL");
const BAKED_KEY: &str = env!("COMFY_GATE_KEY");

/// The gate answers `/api/expand` in ~4s and a 1024² render in ~30s. The
/// ceiling is for a cold model load or a queue in front of us.
const HTTP_TIMEOUT: Duration = Duration::from_secs(180);
const RENDER_TIMEOUT: Duration = Duration::from_secs(900);
const POLL_EVERY: Duration = Duration::from_millis(1500);

/// Which workflow to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// 1024², tiles on both axes. A surface texture or a ZBrush alpha.
    TileSquare,
    /// 1728x576, tiles left/right only. Wraps a ring band; top and bottom are
    /// the band's edges and must not repeat into each other.
    RingBand,
}

impl Preset {
    fn graph(self) -> &'static str {
        match self {
            Preset::TileSquare => include_str!("comfy/tile_square.api.json"),
            Preset::RingBand => include_str!("comfy/ring_band_strip.api.json"),
        }
    }

    /// Name the rig saves under, and what the files are called on the way back.
    fn stem(self) -> &'static str {
        match self {
            Preset::TileSquare => "tile_square",
            Preset::RingBand => "ring_band_strip",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Preset::TileSquare => "Tile (square)",
            Preset::RingBand => "Ring band (wraps around)",
        }
    }
}

/// One render. `Job::new` carries the workflow's own defaults, which are the
/// bench-picked ones — change a field only with a reason.
#[derive(Clone, Debug)]
pub struct Job {
    pub preset: Preset,
    /// The full prompt. `Gate::write_prompt` produces one from a plain idea.
    pub prompt: String,
    pub seed: u64,
    pub steps: u32,
    pub cfg: f32,
    /// BAS-RELIEF LoRA strength. 0.8 is the bench winner; 0.0 disables it and
    /// gives flat line-art, which is not a height map.
    pub relief: f32,
    /// How much of the render's own detail is laid over the depth model's form
    /// (`TileableHeightCombine`). 0.35 default; higher is crisper and noisier.
    pub detail_strength: f32,
    /// Invert the height: black raised instead of white.
    pub invert: bool,
    /// Output size. `None` keeps the preset's own, which is what the crops and
    /// the depth pass were tuned for. A custom size is scaled through the whole
    /// chain, but stay on multiples of 64 and keep the preset's aspect ratio.
    pub size: Option<(u32, u32)>,
    /// Also fetch the lit-relief and colour previews. Off by default: they are
    /// 2 MB and 1.7 MB of PNG that nothing downstream displaces with.
    pub want_previews: bool,
}

impl Job {
    pub fn new(preset: Preset, prompt: impl Into<String>) -> Self {
        Self {
            preset,
            prompt: prompt.into(),
            seed: rand_seed(),
            steps: 30,
            cfg: 5.0,
            relief: 0.8,
            detail_strength: 0.35,
            invert: false,
            size: None,
            want_previews: false,
        }
    }
}

/// What came back. `height_png` is always present; a render that produced no
/// height map is an error, not an empty result.
#[derive(Clone, Debug)]
pub struct Rendered {
    pub job_id: String,
    /// 16-bit PNG. Feed it to `Alpha::from_png16`.
    pub height_png: Vec<u8>,
    pub relief_png: Option<Vec<u8>>,
    pub color_png: Option<Vec<u8>>,
    /// The prompt that actually ran, trigger word included.
    pub prompt: String,
    pub seed: u64,
    pub elapsed: Duration,
}

/// Progress, for a status line. The render itself is one opaque wait — the
/// gate can stream per-step sampler progress over its websocket, but that is a
/// second transport and 30 seconds does not need one.
#[derive(Clone, Debug)]
pub enum Stage {
    Writing,
    Queued { job_id: String },
    Rendering { elapsed: Duration },
    Downloading,
}

pub struct Gate {
    base: String,
    key: String,
    http: reqwest::blocking::Client,
}

impl Gate {
    pub fn new(base: impl Into<String>, key: impl Into<String>) -> Result<Self> {
        Ok(Self {
            base: base.into().trim_end_matches('/').to_string(),
            key: key.into(),
            http: reqwest::blocking::Client::builder()
                .timeout(HTTP_TIMEOUT)
                .build()
                .context("building the http client")?,
        })
    }

    /// The gate this build ships with, or the environment's if it names one.
    ///
    /// `build.rs` compiles the address and key in, so an installed copy needs
    /// no configuration — the key is a gate account that owns nothing, reads
    /// only its own renders and runs one workflow. `COMFY_GATE_KEY` in the
    /// environment still wins, which is how a different rig is pointed at
    /// without a rebuild.
    pub fn installed() -> Result<Self> {
        if std::env::var_os("COMFY_GATE_KEY").is_some() {
            return Self::from_env();
        }
        if BAKED_KEY.is_empty() {
            bail!("no texture server in this build — set COMFY_GATE_KEY to use one");
        }
        let base = if BAKED_URL.is_empty() { DEFAULT_BASE } else { BAKED_URL };
        Self::new(base, BAKED_KEY)
    }

    /// Whether this build can reach a texture server at all, without opening
    /// a connection — what a panel asks before it offers the feature.
    pub fn is_configured() -> bool {
        std::env::var_os("COMFY_GATE_KEY").is_some() || !BAKED_KEY.is_empty()
    }

    /// `COMFY_GATE_KEY` is required; `COMFY_GATE_URL` overrides the default host.
    pub fn from_env() -> Result<Self> {
        let key = std::env::var("COMFY_GATE_KEY")
            .map_err(|_| anyhow!("COMFY_GATE_KEY is not set — no texture server configured"))?;
        let base = std::env::var("COMFY_GATE_URL").unwrap_or_else(|_| DEFAULT_BASE.to_string());
        Self::new(base, key)
    }

    pub fn is_reachable(&self) -> bool {
        self.http
            .get(format!("{}/health", self.base))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Turn a plain idea ("a snake scale texture") into a height-map prompt,
    /// written by the Qwen model on the rig in the `texture` dialect.
    ///
    /// Show the result and let it be edited before rendering: the model is
    /// sampled at temperature 0.4, so the same idea gives a different prompt
    /// each time, and a render is 30 seconds against this call's 4.
    pub fn write_prompt(&self, idea: &str) -> Result<String> {
        let idea = idea.trim();
        if idea.is_empty() {
            bail!("nothing to write a prompt from");
        }
        let resp = self
            .http
            .post(format!("{}/api/expand", self.base))
            .header("x-api-key", &self.key)
            .header("content-type", "application/json")
            .body(serde_json::to_vec(&json!({ "text": idea, "dialect": "texture" }))?)
            .send()
            .context("reaching the prompt writer")?;
        let status = resp.status();
        let body = resp.text().context("reading the prompt writer's reply")?;
        if !status.is_success() {
            bail!("the prompt writer refused ({status}): {}", body.trim());
        }
        let text = assemble_sse(&body);
        let text = clean_reply(&text);
        if text.len() < 40 {
            bail!("the prompt writer returned nothing usable");
        }
        Ok(text)
    }

    pub fn render(&self, job: &Job) -> Result<Rendered> {
        self.render_with_progress(job, &mut |_| {})
    }

    pub fn render_with_progress(
        &self,
        job: &Job,
        on: &mut dyn FnMut(Stage),
    ) -> Result<Rendered> {
        let started = Instant::now();
        let prompt_text = if job.prompt.trim_start().to_lowercase().starts_with("bas-relief") {
            job.prompt.trim().to_string()
        } else {
            format!("{TRIGGER}{}", job.prompt.trim())
        };

        let mut graph: Value =
            serde_json::from_str(job.preset.graph()).context("the embedded workflow is not JSON")?;
        let nodes = graph
            .get_mut("prompt")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| anyhow!("the embedded workflow has no `prompt` object"))?;

        patch(nodes, job, &prompt_text)?;

        let queued: Value = self
            .post_json("/api/prompt", &json!({ "prompt": nodes, "client_id": "ringdesigner" }))
            .context("queueing the render")?;
        if let Some(err) = queued.get("error") {
            bail!("the server rejected the workflow: {err}");
        }
        let job_id = queued
            .get("prompt_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("the server queued nothing"))?
            .to_string();
        on(Stage::Queued { job_id: job_id.clone() });

        let outputs = self.wait(&job_id, on)?;
        on(Stage::Downloading);

        let stem = job.preset.stem();
        let height = self
            .fetch_named(&outputs, &format!("{stem}_height16"))?
            .ok_or_else(|| anyhow!("the render finished without a height map"))?;
        let (relief, color) = if job.want_previews {
            (
                self.fetch_named(&outputs, &format!("{stem}_relief16"))?,
                self.fetch_named(&outputs, &format!("{stem}_color"))?,
            )
        } else {
            (None, None)
        };

        Ok(Rendered {
            job_id,
            height_png: height,
            relief_png: relief,
            color_png: color,
            prompt: prompt_text,
            seed: job.seed,
            elapsed: started.elapsed(),
        })
    }

    /// Poll until the job leaves the queue. The history entry appears only when
    /// the job is done or has failed, so an empty body means "still going".
    fn wait(&self, job_id: &str, on: &mut dyn FnMut(Stage)) -> Result<Vec<OutputRef>> {
        let started = Instant::now();
        while started.elapsed() < RENDER_TIMEOUT {
            let hist: Value = self
                .get_json(&format!("/api/history/{job_id}"))
                .context("asking whether the render finished")?;
            if let Some(entry) = hist.get(job_id) {
                let status = entry.get("status");
                let failed = status
                    .and_then(|s| s.get("status_str"))
                    .and_then(Value::as_str)
                    == Some("error");
                if failed {
                    bail!("the render failed: {}", node_error(entry));
                }
                let done = status
                    .and_then(|s| s.get("completed"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if done {
                    return Ok(collect_outputs(entry));
                }
            }
            on(Stage::Rendering { elapsed: started.elapsed() });
            std::thread::sleep(POLL_EVERY);
        }
        bail!("the render did not finish within {}s", RENDER_TIMEOUT.as_secs())
    }

    /// Outputs are matched by filename, not by node id: the two presets number
    /// their nodes differently and a regenerated workflow may renumber again.
    fn fetch_named(&self, outputs: &[OutputRef], stem: &str) -> Result<Option<Vec<u8>>> {
        let Some(o) = outputs.iter().find(|o| o.filename.contains(stem)) else {
            return Ok(None);
        };
        let resp = self
            .http
            .get(format!("{}/api/view", self.base))
            .header("x-api-key", &self.key)
            .query(&[
                ("filename", o.filename.as_str()),
                ("subfolder", o.subfolder.as_str()),
                ("type", "output"),
            ])
            .send()
            .with_context(|| format!("downloading {}", o.filename))?;
        if !resp.status().is_success() {
            bail!("downloading {} failed: {}", o.filename, resp.status());
        }
        Ok(Some(resp.bytes()?.to_vec()))
    }

    fn post_json(&self, path: &str, body: &Value) -> Result<Value> {
        let resp = self
            .http
            .post(format!("{}{path}", self.base))
            .header("x-api-key", &self.key)
            .header("content-type", "application/json")
            .body(serde_json::to_vec(body)?)
            .send()?;
        let status = resp.status();
        let text = resp.text()?;
        if !status.is_success() {
            bail!("{path} answered {status}: {}", text.trim());
        }
        serde_json::from_str(&text).with_context(|| format!("{path} did not answer JSON"))
    }

    fn get_json(&self, path: &str) -> Result<Value> {
        let resp = self
            .http
            .get(format!("{}{path}", self.base))
            .header("x-api-key", &self.key)
            .send()?;
        let status = resp.status();
        let text = resp.text()?;
        if !status.is_success() {
            bail!("{path} answered {status}: {}", text.trim());
        }
        serde_json::from_str(&text).with_context(|| format!("{path} did not answer JSON"))
    }
}

struct OutputRef {
    filename: String,
    subfolder: String,
}

/// Patch the widgets this crate owns, locating nodes by class rather than by id.
///
/// The save prefixes are deliberately left unqualified: the gate prefixes every
/// save with the key's own namespace, and a prefix that already carries one
/// lands nested inside it.
fn patch(nodes: &mut serde_json::Map<String, Value>, job: &Job, prompt_text: &str) -> Result<()> {
    // The positive encoder is the one the sampler's `positive` input points at.
    let positive_id = nodes
        .iter()
        .find(|(_, n)| class_of(n) == "KSampler")
        .and_then(|(_, n)| n.get("inputs")?.get("positive")?.get(0)?.as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("the workflow has no KSampler with a positive input"))?;

    let old_size = nodes
        .values()
        .find(|n| class_of(n) == "EmptyLatentImage")
        .and_then(|n| {
            let i = n.get("inputs")?;
            Some((i.get("width")?.as_u64()? as u32, i.get("height")?.as_u64()? as u32))
        })
        .ok_or_else(|| anyhow!("the workflow has no EmptyLatentImage"))?;
    let new_size = job.size.unwrap_or(old_size);
    let sx = new_size.0 as f64 / old_size.0 as f64;
    let sy = new_size.1 as f64 / old_size.1 as f64;

    for (id, node) in nodes.iter_mut() {
        let class = class_of(node).to_string();
        let Some(inputs) = node.get_mut("inputs").and_then(Value::as_object_mut) else {
            continue;
        };
        match class.as_str() {
            "CLIPTextEncode" if *id == positive_id => {
                inputs.insert("text".into(), json!(prompt_text));
            }
            "KSampler" => {
                inputs.insert("seed".into(), json!(job.seed));
                inputs.insert("steps".into(), json!(job.steps));
                inputs.insert("cfg".into(), json!(job.cfg));
            }
            "EmptyLatentImage" => {
                inputs.insert("width".into(), json!(new_size.0));
                inputs.insert("height".into(), json!(new_size.1));
            }
            // The relief LoRA. The other two slots stay at 0.0 — which is a
            // true no-op, not a muted node, and muting them would make the
            // graph unqueueable.
            "LoraLoader"
                if inputs
                    .get("lora_name")
                    .and_then(Value::as_str)
                    .is_some_and(|n| n.contains("BAS-RELIEF")) =>
            {
                inputs.insert("strength_model".into(), json!(job.relief));
                inputs.insert("strength_clip".into(), json!(job.relief));
            }
            "TileableHeightCombine" => {
                inputs.insert("detail_strength".into(), json!(job.detail_strength));
                inputs.insert("invert".into(), json!(job.invert));
            }
            // The 3x3 wrap test and the crop back to one tile. Both follow the
            // output size, or the depth pass reads a grid that no longer lines
            // up with the crop and the seam comes back.
            "ImageCrop" => {
                scale(inputs, "width", sx);
                scale(inputs, "height", sy);
                scale(inputs, "x", sx);
                scale(inputs, "y", sy);
            }
            "DA3Inference" => scale(inputs, "resolution", sx.max(sy)),
            "SaveImage" | "SaveImageAdvanced" => {
                if let Some(prefix) = inputs.get("filename_prefix").and_then(Value::as_str) {
                    let bare = prefix.rsplit('/').next().unwrap_or(prefix);
                    inputs.insert("filename_prefix".into(), json!(format!("texture/{bare}")));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn scale(inputs: &mut serde_json::Map<String, Value>, key: &str, by: f64) {
    if (by - 1.0).abs() < f64::EPSILON {
        return;
    }
    if let Some(v) = inputs.get(key).and_then(Value::as_u64) {
        let scaled = ((v as f64 * by).round() as u64).max(1);
        inputs.insert(key.into(), json!(scaled));
    }
}

fn class_of(node: &Value) -> &str {
    node.get("class_type").and_then(Value::as_str).unwrap_or("")
}

fn collect_outputs(entry: &Value) -> Vec<OutputRef> {
    let mut out = Vec::new();
    let Some(outputs) = entry.get("outputs").and_then(Value::as_object) else {
        return out;
    };
    for node in outputs.values() {
        for image in node.get("images").and_then(Value::as_array).into_iter().flatten() {
            let (Some(filename), Some(subfolder)) = (
                image.get("filename").and_then(Value::as_str),
                image.get("subfolder").and_then(Value::as_str),
            ) else {
                continue;
            };
            // Skip the in-graph preview: it lives in `temp`, not in the namespace.
            if image.get("type").and_then(Value::as_str) == Some("temp") {
                continue;
            }
            out.push(OutputRef {
                filename: filename.to_string(),
                subfolder: subfolder.to_string(),
            });
        }
    }
    out
}

fn node_error(entry: &Value) -> String {
    entry
        .get("status")
        .and_then(|s| s.get("messages"))
        .and_then(Value::as_array)
        .map(|msgs| {
            msgs.iter()
                .filter_map(|m| {
                    let kind = m.get(0)?.as_str()?;
                    kind.contains("error").then(|| m.get(1).map(ToString::to_string))?
                })
                .collect::<Vec<_>>()
                .join("; ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "no reason given".into())
}

/// Assemble an OpenAI-style SSE stream into its text. The last frames carry
/// usage and no choices, and the stream ends with `data: [DONE]`.
fn assemble_sse(body: &str) -> String {
    let mut text = String::new();
    for line in body.lines() {
        let Some(payload) = line.strip_prefix("data:") else {
            continue;
        };
        let payload = payload.trim();
        if payload == "[DONE]" {
            break;
        }
        let Ok(frame) = serde_json::from_str::<Value>(payload) else {
            continue;
        };
        for choice in frame.get("choices").and_then(Value::as_array).into_iter().flatten() {
            if let Some(delta) = choice
                .get("delta")
                .and_then(|d| d.get("content"))
                .and_then(Value::as_str)
                .or_else(|| choice.get("text").and_then(Value::as_str))
            {
                text.push_str(delta);
            }
        }
    }
    text
}

/// The preview endpoint streams the model's own output, so the shape and length
/// gates the queue path applies never ran. Strip a thinking block, keep the
/// first line, and drop wrapping quotes.
fn clean_reply(raw: &str) -> String {
    let mut s = raw.trim();
    if let Some(end) = s.find("</think>") {
        s = s[end + "</think>".len()..].trim_start();
    }
    let first = s.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    first.trim_matches('"').trim().to_string()
}

/// Seed from the clock. A render is reproducible from `Rendered::seed`, which
/// is what matters; nothing here needs a crypto source.
fn rand_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64 % 1_000_000_000)
        .unwrap_or(42)
}

/// The Generate window's state: one idea, the prompt written from it, and at
/// most one call in flight.
///
/// Both gate calls block, so they run on a worker and report back over a
/// channel. Writing the prompt and rendering are separate steps on purpose:
/// the model is sampled at temperature 0.4 and a render costs ten times the
/// prompt, so the prompt is shown and editable before any metal is spent.
pub struct TextureGen {
    pub open: bool,
    pub idea: String,
    pub prompt: String,
    pub name: String,
    pub preset: Preset,
    pub invert: bool,
    /// What the worker last said, shown under the buttons.
    pub status: String,
    job: Option<Receiver<Note>>,
}

impl Default for TextureGen {
    fn default() -> Self {
        Self {
            open: false,
            idea: String::new(),
            prompt: String::new(),
            name: String::new(),
            preset: Preset::TileSquare,
            invert: false,
            status: String::new(),
            job: None,
        }
    }
}

/// One message from the worker. The channel closes when it is done.
enum Note {
    Status(String),
    Prompt(String),
    Height(Vec<u8>),
    Failed(String),
}

impl TextureGen {
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// Ask the rig to write a prompt from [`idea`](Self::idea).
    pub fn write_prompt(&mut self, ctx: &egui::Context) {
        let idea = self.idea.trim().to_string();
        if idea.is_empty() || self.busy() {
            return;
        }
        if self.name.trim().is_empty() {
            self.name = default_name(&idea);
        }
        self.spawn(ctx, move |tx| {
            let gate = Gate::installed()?;
            let _ = tx.send(Note::Status("Writing a prompt…".into()));
            let prompt = gate.write_prompt(&idea)?;
            let _ = tx.send(Note::Prompt(prompt));
            Ok(())
        });
    }

    /// Render the prompt into a height map.
    pub fn render(&mut self, ctx: &egui::Context) {
        let prompt = self.prompt.trim().to_string();
        if prompt.is_empty() || self.busy() {
            return;
        }
        let (preset, invert) = (self.preset, self.invert);
        self.spawn(ctx, move |tx| {
            let gate = Gate::installed()?;
            let mut job = Job::new(preset, prompt);
            job.invert = invert;
            let report = tx.clone();
            let out = gate.render_with_progress(&job, &mut |stage| {
                let _ = report.send(Note::Status(match stage {
                    Stage::Writing => "Writing a prompt…".to_string(),
                    Stage::Queued { .. } => "Queued on the rig…".to_string(),
                    Stage::Rendering { elapsed } => format!("Rendering… {}s", elapsed.as_secs()),
                    Stage::Downloading => "Downloading…".to_string(),
                }));
            })?;
            let _ = tx.send(Note::Height(out.height_png));
            Ok(())
        });
    }

    /// Run `body` on a worker, repainting as its notes arrive.
    fn spawn(
        &mut self,
        ctx: &egui::Context,
        body: impl FnOnce(&Sender<Note>) -> Result<()> + Send + 'static,
    ) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.job = Some(rx);
        self.status = "Starting…".into();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            if let Err(e) = body(&tx) {
                let _ = tx.send(Note::Failed(format!("{e:#}")));
            }
            ctx.request_repaint();
        });
    }

    /// Drain the worker. Returns the finished alpha the one time it lands, so
    /// the caller owns inserting it — the same shape `AlphaEditor::ui` uses.
    pub fn poll(&mut self, ctx: &egui::Context) -> Option<ringdesign_core::alpha::Alpha> {
        let Some(rx) = &self.job else { return None };
        let mut alpha = None;
        let mut finished = false;
        loop {
            match rx.try_recv() {
                Ok(Note::Status(s)) => self.status = s,
                Ok(Note::Prompt(p)) => {
                    self.prompt = p;
                    self.status = "Prompt written — edit it, then Render.".into();
                }
                Ok(Note::Height(png)) => {
                    let name = if self.name.trim().is_empty() {
                        default_name(&self.idea)
                    } else {
                        self.name.trim().to_string()
                    };
                    match ringdesign_core::alpha::Alpha::from_png16(&name, &png) {
                        Ok(a) => {
                            self.status = format!("Added \"{name}\" to the library.");
                            alpha = Some(a);
                        }
                        Err(e) => self.status = format!("The render did not decode: {e:#}"),
                    }
                }
                Ok(Note::Failed(e)) => self.status = e,
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    finished = true;
                    break;
                }
            }
        }
        if finished {
            self.job = None;
        } else {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        alpha
    }
}

/// A library name from the idea: its first few words, which is what a person
/// would have typed anyway.
fn default_name(idea: &str) -> String {
    let name: String = idea
        .split_whitespace()
        .take(4)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
        .collect();
    if name.trim().is_empty() { "texture".to_string() } else { name.trim().to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nodes_of(preset: Preset) -> serde_json::Map<String, Value> {
        serde_json::from_str::<Value>(preset.graph())
            .unwrap()
            .get_mut("prompt")
            .unwrap()
            .as_object()
            .unwrap()
            .clone()
    }

    #[test]
    fn both_embedded_graphs_parse_and_carry_their_tiling() {
        for (preset, tiling) in [(Preset::TileSquare, "enable"), (Preset::RingBand, "x_only")] {
            let nodes = nodes_of(preset);
            let tile = nodes
                .values()
                .find(|n| class_of(n) == "SeamlessTile")
                .expect("no SeamlessTile node");
            assert_eq!(
                tile["inputs"]["tiling"].as_str(),
                Some(tiling),
                "{:?} lost its tiling mode",
                preset
            );
            // A tiled VAE decode re-introduces the seam it exists to remove.
            assert!(
                !nodes.values().any(|n| class_of(n) == "VAEDecodeTiled"),
                "{:?} decodes tiled",
                preset
            );
        }
    }

    #[test]
    fn patch_writes_the_prompt_into_the_positive_encoder_only() {
        let mut nodes = nodes_of(Preset::TileSquare);
        let job = Job::new(Preset::TileSquare, "motif");
        patch(&mut nodes, &job, "BAS-RELIEF page, motif").unwrap();
        let positive = nodes
            .values()
            .find(|n| class_of(n) == "KSampler")
            .unwrap()["inputs"]["positive"][0]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(nodes[&positive]["inputs"]["text"], "BAS-RELIEF page, motif");
        let negatives: Vec<_> = nodes
            .values()
            .filter(|n| class_of(n) == "CLIPTextEncode")
            .filter(|n| n["inputs"]["text"] != "BAS-RELIEF page, motif")
            .collect();
        assert_eq!(negatives.len(), 1, "the negative prompt was overwritten");
        assert!(negatives[0]["inputs"]["text"]
            .as_str()
            .unwrap()
            .contains("cast shadows"));
    }

    #[test]
    fn save_prefixes_lose_the_foreign_namespace() {
        let mut nodes = nodes_of(Preset::RingBand);
        patch(&mut nodes, &Job::new(Preset::RingBand, "m"), "m").unwrap();
        for node in nodes.values() {
            if matches!(class_of(node), "SaveImage" | "SaveImageAdvanced") {
                let p = node["inputs"]["filename_prefix"].as_str().unwrap();
                assert!(p.starts_with("texture/"), "{p} is not namespace-free");
                assert_eq!(p.matches('/').count(), 1, "{p} still carries a namespace");
            }
        }
    }

    #[test]
    fn a_custom_size_scales_the_crops_with_it() {
        let mut nodes = nodes_of(Preset::TileSquare);
        let mut job = Job::new(Preset::TileSquare, "m");
        job.size = Some((2048, 2048));
        patch(&mut nodes, &job, "m").unwrap();
        let latent = nodes.values().find(|n| class_of(n) == "EmptyLatentImage").unwrap();
        assert_eq!(latent["inputs"]["width"], 2048);
        let crops: Vec<u64> = nodes
            .values()
            .filter(|n| class_of(n) == "ImageCrop")
            .map(|n| n["inputs"]["x"].as_u64().unwrap())
            .collect();
        assert!(crops.iter().all(|&x| x == 1024), "crop offsets did not follow: {crops:?}");
    }

    #[test]
    fn sse_assembly_survives_the_usage_frame() {
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"seamless \"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"tile\"}}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"total_tokens\":9}}\n\n",
            "data: [DONE]\n\n"
        );
        assert_eq!(assemble_sse(body), "seamless tile");
    }

    /// The whole path, for real: the prompt writer, a render, and the download.
    /// Ignored by default — it spends ~30s of someone's GPU. Run it with
    ///
    /// ```sh
    /// COMFY_GATE_KEY=… cargo test -p ringdesign-gui --bin ringdesigner \
    ///     comfy_texture::tests::live -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "hits the live rig"]
    fn live_round_trip() {
        let gate = Gate::from_env().expect("COMFY_GATE_KEY");
        assert!(gate.is_reachable(), "the gate did not answer /health");

        let prompt = gate.write_prompt("a snake scale texture").unwrap();
        println!("prompt: {prompt}");
        assert!(prompt.contains("seamless"), "not a height-map prompt: {prompt}");
        assert!(
            !prompt.to_lowercase().contains("shadow"),
            "the writer asked for shading: {prompt}"
        );

        let mut job = Job::new(Preset::TileSquare, prompt);
        job.seed = 777;
        let out = gate
            .render_with_progress(&job, &mut |s| println!("{s:?}"))
            .unwrap();
        println!("{} bytes in {:?}", out.height_png.len(), out.elapsed);

        assert_eq!(&out.height_png[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
        // Byte 24 of the IHDR is the bit depth. An 8-bit alpha terraces at
        // displacement depth, which is the whole reason for this pipeline.
        assert_eq!(out.height_png[24], 16, "the height map came back 8-bit");
    }

    #[test]
    fn a_thinking_block_is_not_part_of_the_prompt() {
        let raw = "<think>the user wants scales</think>\nseamless tileable grayscale, scales\nstray";
        assert_eq!(clean_reply(raw), "seamless tileable grayscale, scales");
    }
}
