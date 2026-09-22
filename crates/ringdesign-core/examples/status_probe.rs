//! M4's per-feature status and cache measured: every CAD example and a Court band with a joined
//! bezel and a cut pilot, evaluated cold, then again after an edit to the last feature both
//! without a cache and with the one the first pass filled (best of three, results dropped outside
//! the clock); the cache's estimated bytes against its live heap; then the status tables of
//! documents with a failure injected.
//!
//!     cargo run --example status_probe
use ringdesign_core::{
    AlphaLibrary, BuildParams, RingDesign,
    cad::{self, Attach, BuildCtx, Cache, Component, Document, EdgeRef, FaceRef, Feature, FeatureStatus, Memo, Operation, Placement, Stage},
    mesh, parts, templates,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed},
    },
    time::Instant,
};

/// The system allocator with a count of live bytes.
struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            LIVE.fetch_add(layout.size(), Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) };
        LIVE.fetch_sub(layout.size(), Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, layout, size) };
        if !q.is_null() {
            LIVE.fetch_add(size, Relaxed);
            LIVE.fetch_sub(layout.size(), Relaxed);
        }
        q
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;

const REPEATS: usize = 3;

/// The operation's first dimension grown by 5%, or its first coordinate moved 0.1 mm.
fn nudge(op: &mut Operation) {
    match op {
        Operation::Box { size } => size[0] *= 1.05,
        Operation::Cylinder { radius_mm, .. } | Operation::Sphere { radius_mm } => *radius_mm *= 1.05,
        Operation::Torus { minor_mm, .. } => *minor_mm *= 1.05,
        Operation::TwistedRing { axial_mm, .. } => *axial_mm *= 1.05,
        Operation::Extrude { height_mm, .. } => *height_mm *= 1.05,
        Operation::Revolve { degrees, .. } => *degrees = (*degrees * 0.95).max(1.0),
        Operation::Sweep { path, .. } => {
            if let Some(p) = path.last_mut() {
                p[2] += 0.1;
            }
        }
        Operation::Twist { degrees, .. } => *degrees += 5.0,
        Operation::Fillet { radius_mm, .. } => *radius_mm *= 1.05,
        Operation::Chamfer { distance_mm, .. } => *distance_mm *= 1.05,
        Operation::Shell { thickness_mm, .. } => *thickness_mm *= 1.05,
        Operation::Transform { translation, .. } => translation[0] += 0.1,
        Operation::Band | Operation::Sketch { .. } | Operation::Loft { .. } | Operation::Boolean { .. } => {}
    }
}

/// `d` with its last feature nudged.
fn edited(d: &RingDesign) -> RingDesign {
    let mut d = d.clone();
    if let Some(f) = d.cad.as_mut().and_then(|doc| doc.features.last_mut()) {
        nudge(&mut f.operation);
    }
    d
}

/// Milliseconds `run` takes, its result dropped after the clock stops.
fn timed<T>(run: impl FnOnce() -> T) -> f64 {
    let started = Instant::now();
    let out = run();
    let ms = started.elapsed().as_secs_f64() * 1e3;
    drop(out);
    ms
}

fn counts(cache: &Mutex<Cache>) -> (u64, u64) {
    let c = cache.lock().unwrap();
    (c.hits(), c.misses())
}

/// Best of [`REPEATS`] for a cold pass that fills a fresh cache, the edited design without a
/// cache, and the edited design on the cache the cold pass filled; with the warm pass's hits and
/// misses, the cache's entries, estimated bytes and live heap after the cold pass.
fn cold_plain_warm(cold: impl Fn(Memo) -> f64, plain: impl Fn() -> f64, warm: impl Fn(Memo) -> f64) -> ([f64; 3], (u64, u64), usize, usize, usize) {
    let mut best = [f64::MAX; 3];
    let mut tally = (0, 0);
    let (mut entries, mut estimate, mut heap) = (0, 0, 0);
    for _ in 0..REPEATS {
        let before = LIVE.load(Relaxed);
        let cache = Mutex::new(Cache::default());
        best[0] = best[0].min(cold(Memo::new(&cache)));
        heap = LIVE.load(Relaxed).saturating_sub(before);
        {
            let c = cache.lock().unwrap();
            (entries, estimate) = (c.len(), c.bytes());
        }
        best[1] = best[1].min(plain());
        let (h0, m0) = counts(&cache);
        best[2] = best[2].min(warm(Memo::new(&cache)));
        let (h1, m1) = counts(&cache);
        tally = (h1 - h0, m1 - m0);
    }
    (best, tally, entries, estimate, heap)
}

fn part(id: u64, name: &str, operation: Operation, attach: Attach, height: f64) -> Feature {
    Feature {
        id,
        name: name.into(),
        enabled: true,
        operation,
        component: Component { attach, stage: Stage::Cast, placement: Placement::ring(90.0, height), ..Default::default() },
    }
}

/// The Court band with a bezel joined at the top of the ring and a pilot cut through it.
fn dressed_court() -> RingDesign {
    let mut d = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
    let mut doc = Document::default();
    doc.append(Feature { id: 0, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
    doc.append(part(1, "bezel", Operation::Cylinder { radius_mm: 3.0, height_mm: 2.5 }, Attach::Join, 1.25)).unwrap();
    doc.append(part(2, "pilot", Operation::Cylinder { radius_mm: 1.2, height_mm: 12.0 }, Attach::Cut, 0.0)).unwrap();
    d.cad = Some(doc);
    d
}

fn status_table(e: &cad::Evaluated) {
    for r in &e.features {
        let status = match &r.status {
            FeatureStatus::Ok => "Ok".to_string(),
            FeatureStatus::Suppressed => "Suppressed".to_string(),
            FeatureStatus::Failed(m) => format!("Failed: {m}"),
            FeatureStatus::Skipped(m) => format!("Skipped: {m}"),
        };
        println!("    #{:<3} {:<32} {:>4} faces {:>4} edges  {status}", r.id, r.name, r.faces, r.edges);
    }
}

fn row(name: &str, (best, (hits, misses), entries, estimate, heap): ([f64; 3], (u64, u64), usize, usize, usize), extra: &str) {
    println!(
        "  {:<20} {:>8.1} {:>10.1} {:>9.1} {:>11} {:>7} {:>9.0} {:>9.0}  {extra}",
        name,
        best[0],
        best[1],
        best[2],
        format!("{hits}/{misses}"),
        entries,
        estimate as f64 / 1024.0,
        heap as f64 / 1024.0
    );
}

fn main() {
    let lib = AlphaLibrary::builtin();
    let never = AtomicBool::new(false);
    let ctx = BuildCtx::new(&never);
    let presets = [
        ("preview 256×128", BuildParams { theta_steps: 256, profile_steps: 128, ..BuildParams::default() }),
        ("export 1024×384", BuildParams { theta_steps: 1024, profile_steps: 384, ..BuildParams::default() }),
    ];
    // Lazily built statics and the thread pool, before anything is measured.
    drop(mesh::try_build(&dressed_court(), &lib, presets[0].1));
    for (label, params) in presets {
        println!("== {label}: ms, best of {REPEATS}; hits/misses of the warm pass; the cache after the cold pass");
        println!("  {:<20} {:>8} {:>10} {:>9} {:>11} {:>7} {:>9} {:>9}", "document", "cold", "edit plain", "edit warm", "hits/misses", "entries", "est KB", "heap KB");
        for name in cad::examples::NAMES {
            let d = cad::examples::design(name).unwrap();
            let changed = edited(&d);
            let e = cad::evaluate(&changed, &lib, params).unwrap();
            assert!(e.failures().is_empty(), "{name}: {:?}", e.failures());
            let measured = cold_plain_warm(
                |memo| timed(|| cad::evaluate_memo(&d, &lib, params, &ctx, memo).unwrap()),
                || timed(|| cad::evaluate_with(&changed, &lib, params, &ctx).unwrap()),
                |memo| timed(|| cad::evaluate_memo(&changed, &lib, params, &ctx, memo).unwrap()),
            );
            row(name, measured, &format!("{} features", d.cad.as_ref().unwrap().features.len()));
        }
        // The Court band dressed: the parts evaluated on the built surface, then resolved into it.
        let d = dressed_court();
        let changed = edited(&d);
        let mut band_only = d.clone();
        band_only.cad = None;
        let band = mesh::try_build(&band_only, &lib, params).unwrap();
        let band_ms = (0..REPEATS).map(|_| timed(|| mesh::try_build(&band_only, &lib, params).unwrap())).fold(f64::MAX, f64::min);
        let epoch_ms = (0..REPEATS).map(|_| timed(|| cad::surface_epoch(&band.mesh))).fold(f64::MAX, f64::min);
        let epoch = cad::surface_epoch(&band.mesh);
        let on = BuildCtx::new(&never).with_surface(&band.mesh);
        let measured = cold_plain_warm(
            |memo| timed(|| cad::evaluate_memo(&d, &lib, params, &on, memo.with_epoch(epoch)).unwrap()),
            || timed(|| cad::evaluate_with(&changed, &lib, params, &on).unwrap()),
            |memo| timed(|| cad::evaluate_memo(&changed, &lib, params, &on, memo.with_epoch(epoch)).unwrap()),
        );
        row("Court + bezel/pilot", measured, &format!("band {band_ms:.0} ms, {} faces; epoch {epoch_ms:.2} ms", band.mesh.faces.len()));
        // The whole resolve stage, booleans included, on a fresh band each time.
        let resolve = |design: &RingDesign, memo: Memo| {
            let mut built = mesh::try_build(&band_only, &lib, params).unwrap();
            let started = Instant::now();
            let r = parts::resolve_with(design, &lib, params, &BuildCtx::new(&never), memo, &mut built).unwrap();
            let ms = started.elapsed().as_secs_f64() * 1e3;
            assert!(r.joined + r.cut == 2 && r.notes.is_empty(), "{:?}", r.notes);
            ms
        };
        let measured = cold_plain_warm(|memo| resolve(&d, memo), || resolve(&changed, Memo::default()), |memo| resolve(&changed, memo));
        row("  resolve stage", measured, "1 joined, 1 cut");
    }

    // A failure injected into the dressed Court band: a fillet on an edge the bezel does not have,
    // a chamfer on the fillet, and a sketch on the fillet's face swept by an extrusion.
    println!("\n== status with a failure injected (preview)");
    let params = presets[0].1;
    let mut d = dressed_court();
    let doc = d.cad.as_mut().unwrap();
    doc.append(Feature { id: 3, name: "bezel fillet".into(), enabled: true, operation: Operation::Fillet { source: 1, edges: vec![EdgeRef::bare(999)], radius_mm: 0.3 }, component: Component::default() }).unwrap();
    doc.append(Feature { id: 4, name: "rim chamfer".into(), enabled: true, operation: Operation::Chamfer { source: 3, edges: vec![EdgeRef::bare(0)], base_face: FaceRef::bare(0), distance_mm: 0.2 }, component: Component::default() }).unwrap();
    let mut sketch = ringdesign_core::sketch::Sketch::rectangle(1.0, 1.0);
    sketch.plane.on_face = Some(ringdesign_core::sketch::FaceAnchor { feature: 3, face: FaceRef::bare(0) });
    doc.append(Feature { id: 5, name: "crest sketch".into(), enabled: true, operation: Operation::Sketch { sketch }, component: Component::default() }).unwrap();
    doc.append(Feature { id: 6, name: "crest boss".into(), enabled: true, operation: Operation::Extrude { sketch: cad::Profile::Feature { feature: 5 }, height_mm: 0.5, draft_deg: 0.0 }, component: Component::default() }).unwrap();
    let started = Instant::now();
    let built = mesh::try_build(&d, &lib, params).unwrap();
    println!(
        "  Court band, built in {:.0} ms: {} joined, {} cut, watertight {}",
        started.elapsed().as_secs_f64() * 1e3,
        built.parts.joined,
        built.parts.cut,
        built.report.validation.watertight
    );
    let e = built.parts.evaluated.as_ref().unwrap();
    status_table(e);
    println!("    first error: {}", e.first_error().unwrap_or_default());
    for n in &built.parts.notes {
        println!("    note: {n}");
    }
    // The same on a ring of parts only: the solitaire's setting stock replaced by a failing fillet.
    let mut d = cad::examples::design("solitaire").unwrap();
    let doc = d.cad.as_mut().unwrap();
    doc.features[1].name = "Setting stock fillet".into();
    doc.features[1].operation = Operation::Fillet { source: 1, edges: vec![EdgeRef::bare(999)], radius_mm: 0.3 };
    let started = Instant::now();
    let built = mesh::try_build(&d, &lib, params).unwrap();
    println!(
        "  solitaire, built in {:.0} ms: {} part separate, {} reference, origin on {} of {} vertices",
        started.elapsed().as_secs_f64() * 1e3,
        built.parts.separate,
        built.parts.references,
        built.mesh.origin.len(),
        built.mesh.vertices.len()
    );
    status_table(built.parts.evaluated.as_ref().unwrap());
    for n in &built.parts.notes {
        println!("    note: {n}");
    }
}
