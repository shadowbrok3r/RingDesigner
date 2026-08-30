//! Alphas — grayscale height maps that tile across the band surface.
//!
//! An alpha is a `width x height` grid of 0..1 samples. 0 leaves the base
//! surface alone; 1 displaces by the owning layer's full height.

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::field::smoothstep;

/// Resolution the built-in patterns are rendered at.
const BUILTIN_SIZE: usize = 256;

/// Longest edge an alpha is kept at. Sources above this are downscaled on load.
/// One f32 per pixel, so this is 1 MB per square alpha.
pub const MAX_ALPHA_EDGE: usize = 512;

/// Longest edge accepted at all. Above this a file is rejected from its header
/// rather than decoded, so a small highly-compressible image cannot expand into
/// gigabytes before anyone can downscale it.
pub const HARD_MAX_ALPHA_EDGE: usize = 8192;

/// Shared decode path for [`Alpha::load`] and [`Alpha::from_bytes`].
///
/// The header limits reject an oversized image before its decode buffer is ever allocated, and the
/// downscale happens before the f32 expansion, never after: one f32 per pixel means a 16000x16000
/// image from a 1 MB file would otherwise retain a gigabyte.
fn decode<R: std::io::BufRead + std::io::Seek>(
    mut reader: image::ImageReader<R>,
    name: String,
) -> anyhow::Result<Alpha> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(HARD_MAX_ALPHA_EDGE as u32);
    limits.max_image_height = Some(HARD_MAX_ALPHA_EDGE as u32);
    reader.limits(limits);

    let decoded = reader.decode()?;
    let (sw, sh) = (decoded.width(), decoded.height());
    if sw == 0 || sh == 0 {
        anyhow::bail!("{name} has zero extent");
    }

    let max_edge = MAX_ALPHA_EDGE as u32;
    let luma = if sw > max_edge || sh > max_edge {
        let scale = (max_edge as f64 / sw.max(sh) as f64).min(1.0);
        let tw = ((sw as f64 * scale).round() as u32).max(1);
        let th = ((sh as f64 * scale).round() as u32).max(1);
        image::imageops::resize(
            &decoded.into_luma8(),
            tw,
            th,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        decoded.into_luma8()
    };

    let (w, h) = (luma.width() as usize, luma.height() as usize);
    let data: Vec<f32> = luma.into_raw().into_iter().map(|p| p as f32 / 255.0).collect();
    Ok(Alpha::new(name, w, h, data))
}

/// Ceiling on library entries, so many files cannot multiply the per-file cost.
pub const MAX_LIBRARY_ENTRIES: usize = 1024;

#[derive(Clone, Debug, Default)]
pub struct Alpha {
    pub name: String,
    pub width: usize,
    pub height: usize,
    /// Row-major, `width * height` samples in 0..1.
    pub data: Vec<f32>,
}

impl Alpha {
    pub fn new(name: impl Into<String>, width: usize, height: usize, data: Vec<f32>) -> Self {
        debug_assert_eq!(data.len(), width * height);
        Self { name: name.into(), width, height, data }
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0 || self.data.is_empty()
    }

    /// Bilinear sample with clamped edges. `x`/`y` are 0..1 over the image.
    #[inline]
    pub fn sample(&self, x: f64, y: f64) -> f32 {
        if self.data.len() < self.width * self.height || self.is_empty() {
            return 0.0;
        }
        let (w, h) = (self.width, self.height);
        let cx = if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.0 };
        let cy = if y.is_finite() { y.clamp(0.0, 1.0) } else { 0.0 };
        let gx = cx * (w - 1) as f64;
        let gy = cy * (h - 1) as f64;
        let ix0 = gx as usize;
        let iy0 = gy as usize;
        let tx = (gx - ix0 as f64) as f32;
        let ty = (gy - iy0 as f64) as f32;
        let ix1 = (ix0 + 1).min(w - 1);
        let iy1 = (iy0 + 1).min(h - 1);
        self.lerp4(ix0, ix1, iy0, iy1, tx, ty)
    }

    /// Bilinear sample with wrapping, for alphas that repeat continuously.
    #[inline]
    pub fn sample_wrapped(&self, x: f64, y: f64) -> f32 {
        if self.data.len() < self.width * self.height || self.is_empty() {
            return 0.0;
        }
        let (w, h) = (self.width, self.height);
        let mut fx = x - x.floor();
        let mut fy = y - y.floor();
        if !(0.0..1.0).contains(&fx) {
            fx = 0.0;
        }
        if !(0.0..1.0).contains(&fy) {
            fy = 0.0;
        }
        let gx = fx * w as f64;
        let gy = fy * h as f64;
        let ix0 = (gx as usize).min(w - 1);
        let iy0 = (gy as usize).min(h - 1);
        let tx = (gx - ix0 as f64) as f32;
        let ty = (gy - iy0 as f64) as f32;
        let ix1 = if ix0 + 1 == w { 0 } else { ix0 + 1 };
        let iy1 = if iy0 + 1 == h { 0 } else { iy0 + 1 };
        self.lerp4(ix0, ix1, iy0, iy1, tx, ty)
    }

    #[inline]
    fn lerp4(&self, ix0: usize, ix1: usize, iy0: usize, iy1: usize, tx: f32, ty: f32) -> f32 {
        let r0 = iy0 * self.width;
        let r1 = iy1 * self.width;
        let a = self.data[r0 + ix0];
        let b = self.data[r0 + ix1];
        let c = self.data[r1 + ix0];
        let d = self.data[r1 + ix1];
        let top = a + (b - a) * tx;
        let bot = c + (d - c) * tx;
        top + (bot - top) * ty
    }

    /// Load a PNG/JPEG/BMP and convert to luminance, downscaled to
    /// [`MAX_ALPHA_EDGE`].
    ///
    /// An alpha is a height map sampled far below its own resolution, so there
    /// is nothing to gain from keeping a large source and a great deal to lose:
    /// one f32 per pixel means a 16000x16000 image from a 1 MB file would
    /// retain a gigabyte.
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Alpha> {
        let path = path.as_ref();
        let reader = image::ImageReader::open(path)?.with_guessed_format()?;
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "alpha".to_string());
        decode(reader, name).map_err(|e| e.context(format!("{}", path.display())))
    }

    /// Decode an image already in memory.
    ///
    /// Android hands files over as bytes — `MediaStore` returns a `Vec<u8>`, and a `content://`
    /// URI is not a path at all — so there is nothing for [`load`](Self::load) to open. Same
    /// limits, same downscale, same 0..1 expansion.
    pub fn from_bytes(name: impl Into<String>, bytes: &[u8]) -> anyhow::Result<Alpha> {
        let reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
        decode(reader, name.into())
    }

    /// 16-bit grayscale PNG of the height data, for embedding in a design file.
    pub fn to_png16(&self) -> anyhow::Result<Vec<u8>> {
        anyhow::ensure!(!self.is_empty(), "{} is empty", self.name);
        let mut img = image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::new(
            self.width as u32,
            self.height as u32,
        );
        for (px, v) in img.pixels_mut().zip(&self.data) {
            px.0[0] = (v.clamp(0.0, 1.0) * 65535.0).round() as u16;
        }
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png)?;
        Ok(out.into_inner())
    }

    /// Decode a [`to_png16`](Self::to_png16) payload, keeping 16-bit precision.
    ///
    /// An oversized payload falls back to the standard import path and its
    /// downscale, so a hostile design file cannot expand past the library caps.
    pub fn from_png16(name: impl Into<String>, bytes: &[u8]) -> anyhow::Result<Alpha> {
        let name = name.into();
        let mut reader =
            image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(HARD_MAX_ALPHA_EDGE as u32);
        limits.max_image_height = Some(HARD_MAX_ALPHA_EDGE as u32);
        reader.limits(limits);
        let decoded = reader.decode()?;
        let (sw, sh) = (decoded.width(), decoded.height());
        if sw == 0 || sh == 0 {
            anyhow::bail!("{name} has zero extent");
        }
        if sw.max(sh) > MAX_ALPHA_EDGE as u32 {
            let reader =
                image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
            return decode(reader, name);
        }
        let luma = decoded.into_luma16();
        let (w, h) = (luma.width() as usize, luma.height() as usize);
        let data = luma.into_raw().into_iter().map(|p| p as f32 / 65535.0).collect();
        Ok(Alpha::new(name, w, h, data))
    }

    /// RGBA8 preview downscaled to fit `max_edge`, as `(width, height, bytes)`.
    ///
    /// The library grid draws thumbnails at a few dozen pixels, so uploading a
    /// full-resolution texture per entry wastes both a large transient copy and
    /// the VRAM it lands in.
    pub fn thumbnail_rgba8(&self, max_edge: usize) -> (usize, usize, Vec<u8>) {
        if self.is_empty() {
            return (0, 0, Vec::new());
        }
        let max_edge = max_edge.max(1);
        let long = self.width.max(self.height);
        if long <= max_edge {
            return (self.width, self.height, self.rgba8());
        }
        let scale = max_edge as f64 / long as f64;
        let tw = ((self.width as f64 * scale).round() as usize).max(1);
        let th = ((self.height as f64 * scale).round() as usize).max(1);
        let mut out = Vec::with_capacity(tw * th * 4);
        for j in 0..th {
            let y = (j as f64 + 0.5) / th as f64;
            for i in 0..tw {
                let x = (i as f64 + 0.5) / tw as f64;
                let b = (self.sample(x, y).clamp(0.0, 1.0) * 255.0).round() as u8;
                out.extend_from_slice(&[b, b, b, 255]);
            }
        }
        (tw, th, out)
    }

    /// RGBA8 preview bytes for a texture upload, row-major, `width * height * 4`.
    pub fn rgba8(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.width * self.height * 4);
        for j in 0..self.height {
            for i in 0..self.width {
                let v = self.data.get(j * self.width + i).copied().unwrap_or(0.0);
                let b = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                out.extend_from_slice(&[b, b, b, 255]);
            }
        }
        out
    }

    /// Rescale so the darkest sample is 0 and the brightest is 1.
    pub fn normalized(&self) -> Alpha {
        if self.is_empty() || self.data.len() != self.width * self.height {
            return self.clone();
        }
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for &v in &self.data {
            if v.is_finite() {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        if !lo.is_finite() || !hi.is_finite() || (hi - lo) < 1e-6 {
            return self.clone();
        }
        let inv = 1.0 / (hi - lo);
        let data = self.data.iter().map(|&v| ((v - lo) * inv).clamp(0.0, 1.0)).collect();
        Alpha::new(self.name.clone(), self.width, self.height, data)
    }

    /// Cross-fade opposite edges so adjacent tiles butt without a visible seam.
    /// `blend` is the fraction of the image blended, 0..0.5.
    /// Cross-fade the opposite edges of a tile into each other so an imported
    /// image tiles without a seam.
    ///
    /// **Nothing in the app calls this.** CLAUDE.md named it as the mechanism
    /// imports go through and it never was one; an imported image is used as
    /// drawn, seam and all. It is kept because it is correct and tested, and
    /// because the alpha-operator work (M16) wants it as one of the ops in
    /// `alpha.transform`, where it belongs — a per-alpha choice rather than
    /// something done silently to everything that comes in.
    pub fn make_seamless(&self, blend: f64) -> Alpha {
        if self.is_empty() || self.data.len() != self.width * self.height {
            return self.clone();
        }
        let (w, h) = (self.width, self.height);
        let b = if blend.is_finite() { blend.clamp(0.0, 0.5) } else { 0.0 };
        let bx = ((b * w as f64) as usize).min(w / 2);
        let by = ((b * h as f64) as usize).min(h / 2);
        let mut data = self.data.clone();

        if bx > 0 {
            let src = data.clone();
            for j in 0..h {
                let row = j * w;
                for i in 0..w {
                    let d = i.min(w - 1 - i);
                    if d >= bx {
                        continue;
                    }
                    let t = 0.5 * (1.0 - d as f32 / bx as f32);
                    let mirror = src[row + (w - 1 - i)];
                    data[row + i] = src[row + i] * (1.0 - t) + mirror * t;
                }
            }
        }
        if by > 0 {
            let src = data.clone();
            for j in 0..h {
                let d = j.min(h - 1 - j);
                if d >= by {
                    continue;
                }
                let t = 0.5 * (1.0 - d as f32 / by as f32);
                let row = j * w;
                let mrow = (h - 1 - j) * w;
                for i in 0..w {
                    data[row + i] = src[row + i] * (1.0 - t) + src[mrow + i] * t;
                }
            }
        }
        Alpha::new(self.name.clone(), w, h, data)
    }

    /// Morphological opening by a cone, so no wall in the baked relief exceeds
    /// `max_wall_deg` at the given scale — the rolling-ball fairing the signet
    /// body uses, applied in 2D to a texture. Peaks too thin to support at
    /// Signed Euclidean distance to the alpha's 0.5 crossing, in **pixels**,
    /// positive inside the ink. Derived data for the mm-true edge mode: a
    /// layer turns it into millimetres with its own cell scale, so the same
    /// mask carries the same bevel at any tile size. Computed on a 3-wide
    /// x-tiling so a seamless source stays seamless; never persisted — the
    /// source alpha is, and this regenerates from it.
    pub fn signed_distance_px(&self) -> Alpha {
        let (w, h) = (self.width, self.height);
        let name = sdf_name(&self.name);
        if self.is_empty() {
            return Alpha::new(name, 0, 0, Vec::new());
        }
        const FAR: f32 = 1e12;
        let w3 = w * 3;
        // Squared distance to the nearest inside pixel and nearest outside
        // pixel; whichever side a pixel is on, one of the two is zero.
        let mut to_ink = vec![FAR; w3 * h];
        let mut to_ground = vec![FAR; w3 * h];
        for y in 0..h {
            for x3 in 0..w3 {
                let v = self.data[y * w + x3 % w];
                if v >= 0.5 {
                    to_ink[y * w3 + x3] = 0.0;
                } else {
                    to_ground[y * w3 + x3] = 0.0;
                }
            }
        }
        for grid in [&mut to_ink, &mut to_ground] {
            edt_squared(grid, w3, h);
        }
        let mut out = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let i = y * w3 + (x + w);
                out.push(to_ground[i].sqrt() - to_ink[i].sqrt());
            }
        }
        Alpha::new(name, w, h, out)
    }

    /// that slope are removed rather than left as unmouldable cliffs.
    ///
    /// `mm_per_px` is the tile cell's pitch per texel and `relief_mm` the
    /// layer height the values scale to; together they turn the angle into a
    /// value-per-texel slope. Runs on a 3x3 tiling of the image so the result
    /// stays seamless.
    pub fn draft_limited(&self, max_wall_deg: f64, mm_per_px: (f64, f64), relief_mm: f64) -> Alpha {
        let (w, h) = (self.width, self.height);
        if self.is_empty() || relief_mm <= 1e-6 {
            return self.clone();
        }
        let t = max_wall_deg.clamp(5.0, 89.0).to_radians().tan();
        let ax = (t * mm_per_px.0.max(1e-9) / relief_mm) as f32;
        let ay = (t * mm_per_px.1.max(1e-9) / relief_mm) as f32;
        // A limit no gradient can exceed changes nothing.
        if ax >= 1.0 && ay >= 1.0 {
            return self.clone();
        }
        let ad = (ax * ax + ay * ay).sqrt();

        // 3x3 tiled working copy, so slopes propagate across the seams.
        let (bw, bh) = (w * 3, h * 3);
        let mut buf = vec![0.0f32; bw * bh];
        for j in 0..bh {
            for i in 0..bw {
                buf[j * bw + i] = self.data[(j % h) * w + (i % w)];
            }
        }

        let chamfer = |buf: &mut [f32], erode: bool| {
            let pick = |a: f32, b: f32| if erode { a.min(b) } else { a.max(b) };
            let step = |base: f32, cost: f32| if erode { base + cost } else { base - cost };
            for j in 0..bh {
                for i in 0..bw {
                    let mut v = buf[j * bw + i];
                    if i > 0 {
                        v = pick(v, step(buf[j * bw + i - 1], ax));
                    }
                    if j > 0 {
                        v = pick(v, step(buf[(j - 1) * bw + i], ay));
                        if i > 0 {
                            v = pick(v, step(buf[(j - 1) * bw + i - 1], ad));
                        }
                        if i + 1 < bw {
                            v = pick(v, step(buf[(j - 1) * bw + i + 1], ad));
                        }
                    }
                    buf[j * bw + i] = v;
                }
            }
            for j in (0..bh).rev() {
                for i in (0..bw).rev() {
                    let mut v = buf[j * bw + i];
                    if i + 1 < bw {
                        v = pick(v, step(buf[j * bw + i + 1], ax));
                    }
                    if j + 1 < bh {
                        v = pick(v, step(buf[(j + 1) * bw + i], ay));
                        if i + 1 < bw {
                            v = pick(v, step(buf[(j + 1) * bw + i + 1], ad));
                        }
                        if i > 0 {
                            v = pick(v, step(buf[(j + 1) * bw + i - 1], ad));
                        }
                    }
                    buf[j * bw + i] = v;
                }
            }
        };
        chamfer(&mut buf, true);
        chamfer(&mut buf, false);

        let mut out = Vec::with_capacity(w * h);
        for j in 0..h {
            for i in 0..w {
                out.push(buf[(j + h) * bw + i + w].clamp(0.0, 1.0));
            }
        }
        Alpha::new(format!("{} drafted", self.name), w, h, out)
    }

    /// Sample with the layer's contrast/bias/invert response curve applied.
    pub fn shaped(&self, raw: f32, contrast: f64, bias: f64, invert: bool) -> f64 {
        let mut t = raw as f64;
        if invert {
            t = 1.0 - t;
        }
        t = (t + bias).clamp(0.0, 1.0);
        let c = contrast.clamp(0.05, 8.0);
        if (c - 1.0).abs() > 1e-9 {
            t = t.powf(c);
        }
        t.clamp(0.0, 1.0)
    }
}

impl Alpha {
    /// The finest features in the mask, in pixels, as `(ink, gap)`: the
    /// diameter of the smallest strokes and of the smallest gaps between
    /// them, each read as the opening diameter at which a tenth of that
    /// phase's area disappears — granulometry on the distance field,
    /// bisected over the radius, on a 3x3 tiling so a seamless mask reads
    /// seamless. `None` for an empty or single-phase mask. Cached by content.
    pub fn min_feature_px(&self) -> Option<(f64, f64)> {
        use std::collections::HashMap;
        use std::hash::{Hash, Hasher};
        use std::sync::{Mutex, OnceLock};
        static CACHE: OnceLock<Mutex<HashMap<u64, Option<(f64, f64)>>>> = OnceLock::new();
        if self.is_empty() {
            return None;
        }
        let mut h = std::hash::DefaultHasher::new();
        (self.width, self.height).hash(&mut h);
        for v in &self.data {
            v.to_bits().hash(&mut h);
        }
        let key = h.finish();
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Some(hit) = cache.lock().ok().and_then(|c| c.get(&key).copied()) {
            return hit;
        }
        let got = self.measure_features();
        if let Ok(mut c) = cache.lock() {
            if c.len() > 512 {
                c.clear();
            }
            c.insert(key, got);
        }
        got
    }

    fn measure_features(&self) -> Option<(f64, f64)> {
        const LOST: f64 = 0.10;
        let (w, h) = (self.width, self.height);
        let (bw, bh) = (w * 3, h * 3);
        let ink: Vec<bool> = (0..bw * bh).map(|i| self.data[(i / bw % h) * w + (i % bw % w)] >= 0.5).collect();
        let n_ink = ink.iter().filter(|&&b| b).count();
        if n_ink == 0 || n_ink == ink.len() {
            return None;
        }
        let dist_to = |set: &[bool]| -> Vec<f32> {
            let mut g: Vec<f32> = set.iter().map(|&b| if b { 0.0 } else { 1e12 }).collect();
            edt_squared(&mut g, bw, bh);
            g.iter_mut().for_each(|v| *v = v.sqrt());
            g
        };
        let ground: Vec<bool> = ink.iter().map(|&b| !b).collect();
        let d_ink = dist_to(&ground);
        let d_ground = dist_to(&ink);
        // Share of a phase (centre tile) that an opening by disc radius r removes.
        let loss = |phase: &[bool], d: &[f32], r: f32| -> f64 {
            let eroded: Vec<bool> = d.iter().map(|&v| v >= r).collect();
            let back = dist_to(&eroded);
            let (mut lost, mut all) = (0usize, 0usize);
            for y in h..2 * h {
                for x in w..2 * w {
                    let i = y * bw + x;
                    if phase[i] {
                        all += 1;
                        if back[i] > r {
                            lost += 1;
                        }
                    }
                }
            }
            if all == 0 { 1.0 } else { lost as f64 / all as f64 }
        };
        let feature = |phase: &[bool], d: &[f32]| -> f64 {
            let r_max = (w.min(h) as f32) * 0.5;
            if loss(phase, d, r_max) < LOST {
                return 2.0 * r_max as f64;
            }
            let (mut lo, mut hi) = (0.0f32, r_max);
            while hi - lo > 0.25 {
                let mid = 0.5 * (lo + hi);
                if loss(phase, d, mid) >= LOST { hi = mid } else { lo = mid }
            }
            2.0 * hi as f64
        };
        Some((feature(&ink, &d_ink), feature(&ground, &d_ground)))
    }
}

/// Library name a mask's derived signed-distance field lands under.
pub fn sdf_name(name: &str) -> String {
    format!("{name}##sdf")
}

/// In-place exact squared Euclidean distance transform (Felzenszwalb &
/// Huttenlocher): a 1D lower-envelope pass down every column, then along
/// every row.
fn edt_squared(grid: &mut [f32], w: usize, h: usize) {
    let n = w.max(h);
    let mut f = vec![0.0f32; n];
    let mut d = vec![0.0f32; n];
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f32; n + 1];

    let pass = |f: &[f32], d: &mut [f32], n: usize, v: &mut [usize], z: &mut [f32]| {
        let mut k = 0usize;
        v[0] = 0;
        z[0] = f32::NEG_INFINITY;
        z[1] = f32::INFINITY;
        for q in 1..n {
            loop {
                let p = v[k];
                let s = ((f[q] + (q * q) as f32) - (f[p] + (p * p) as f32))
                    / (2.0 * (q as f32 - p as f32));
                if s <= z[k] {
                    if k == 0 {
                        break;
                    }
                    k -= 1;
                } else {
                    k += 1;
                    v[k] = q;
                    z[k] = s;
                    z[k + 1] = f32::INFINITY;
                    break;
                }
            }
        }
        let mut k = 0usize;
        for q in 0..n {
            while z[k + 1] < q as f32 {
                k += 1;
            }
            let p = v[k] as f32;
            d[q] = (q as f32 - p) * (q as f32 - p) + f[p as usize];
        }
    };

    for x in 0..w {
        for y in 0..h {
            f[y] = grid[y * w + x];
        }
        pass(&f, &mut d, h, &mut v, &mut z);
        for y in 0..h {
            grid[y * w + x] = d[y];
        }
    }
    for y in 0..h {
        f[..w].copy_from_slice(&grid[y * w..y * w + w]);
        pass(&f, &mut d, w, &mut v, &mut z);
        grid[y * w..y * w + w].copy_from_slice(&d[..w]);
    }
}

/// Built-in procedurally generated alphas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Procedural {
    Rope,
    Braid,
    Basketweave,
    Chevron,
    Herringbone,
    Scales,
    GreekKey,
    CelticKnot,
    Floral,
    Hammered,
    Waves,
    Diamonds,
    Beads,
    Feather,
    Bark,
    Nugget,
    Rosette,
    Barleycorn,
    Moire,
    Starburst,
    Florentine,
    GuillocheWeave,
    Voronoi,
    Trellis,
    Honeycomb,
    Pyramids,
    Argyle,
    Lattice,
}

impl Procedural {
    pub const ALL: &'static [Procedural] = &[
        Procedural::Rope,
        Procedural::Braid,
        Procedural::Basketweave,
        Procedural::Chevron,
        Procedural::Herringbone,
        Procedural::Scales,
        Procedural::GreekKey,
        Procedural::CelticKnot,
        Procedural::Floral,
        Procedural::Hammered,
        Procedural::Waves,
        Procedural::Diamonds,
        Procedural::Beads,
        Procedural::Feather,
        Procedural::Bark,
        Procedural::Nugget,
        Procedural::Rosette,
        Procedural::Barleycorn,
        Procedural::Moire,
        Procedural::Starburst,
        Procedural::Florentine,
        Procedural::GuillocheWeave,
        Procedural::Voronoi,
        Procedural::Trellis,
        Procedural::Honeycomb,
        Procedural::Pyramids,
        Procedural::Argyle,
        Procedural::Lattice,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Procedural::Rope => "Rope",
            Procedural::Braid => "Braid",
            Procedural::Basketweave => "Basketweave",
            Procedural::Chevron => "Chevron",
            Procedural::Herringbone => "Herringbone",
            Procedural::Scales => "Scales",
            Procedural::GreekKey => "Greek Key",
            Procedural::CelticKnot => "Celtic Knot",
            Procedural::Floral => "Floral",
            Procedural::Hammered => "Hammered",
            Procedural::Waves => "Waves",
            Procedural::Diamonds => "Diamonds",
            Procedural::Beads => "Beads",
            Procedural::Feather => "Feather",
            Procedural::Bark => "Bark",
            Procedural::Nugget => "Nugget",
            Procedural::Rosette => "Rosette",
            Procedural::Barleycorn => "Barleycorn",
            Procedural::Moire => "Moire",
            Procedural::Starburst => "Starburst",
            Procedural::Florentine => "Florentine",
            Procedural::GuillocheWeave => "Guilloche",
            Procedural::Voronoi => "Voronoi",
            Procedural::Trellis => "Trellis",
            Procedural::Honeycomb => "Honeycomb",
            Procedural::Pyramids => "Pyramids",
            Procedural::Argyle => "Argyle",
            Procedural::Lattice => "Lattice",
        }
    }

    /// Render this pattern at `size x size`. Must tile seamlessly in both axes.
    pub fn generate(self, size: usize) -> Alpha {
        let n = size.max(1);
        let mut raw = vec![0.0f64; n * n];
        let inv = 1.0 / n as f64;
        for j in 0..n {
            let y = j as f64 * inv;
            for i in 0..n {
                let x = i as f64 * inv;
                let v = match self {
                    Procedural::Rope => rope(x, y),
                    Procedural::Braid => braid(x, y),
                    Procedural::Basketweave => basketweave(x, y),
                    Procedural::Chevron => chevron(x, y),
                    Procedural::Herringbone => herringbone(x, y),
                    Procedural::Scales => scales(x, y),
                    Procedural::GreekKey => greek_key(x, y),
                    Procedural::CelticKnot => celtic_knot(x, y),
                    Procedural::Floral => floral(x, y),
                    Procedural::Hammered => hammered(x, y),
                    Procedural::Waves => waves(x, y),
                    Procedural::Diamonds => diamonds(x, y),
                    Procedural::Beads => beads(x, y),
                    Procedural::Feather => feather(x, y),
                    Procedural::Bark => bark(x, y),
                    Procedural::Nugget => nugget(x, y),
                    Procedural::Rosette => rosette(x, y),
                    Procedural::Barleycorn => barleycorn(x, y),
                    Procedural::Moire => moire(x, y),
                    Procedural::Starburst => starburst(x, y),
                    Procedural::Florentine => florentine(x, y),
                    Procedural::GuillocheWeave => guilloche_weave(x, y),
                    Procedural::Voronoi => voronoi(x, y),
                    Procedural::Trellis => trellis(x, y),
                    Procedural::Honeycomb => honeycomb(x, y),
                    Procedural::Pyramids => pyramids(x, y),
                    Procedural::Argyle => argyle(x, y),
                    Procedural::Lattice => lattice(x, y),
                };
                raw[j * n + i] = if v.is_finite() { v } else { 0.0 };
            }
        }
        Alpha::new(self.label(), n, n, rescale01(&raw))
    }

    /// Sample the pattern function at unit coordinates, for [`ProcRecipe`].
    fn at(self, x: f64, y: f64) -> f64 {
        match self {
            Procedural::Rope => rope(x, y),
            Procedural::Braid => braid(x, y),
            Procedural::Basketweave => basketweave(x, y),
            Procedural::Chevron => chevron(x, y),
            Procedural::Herringbone => herringbone(x, y),
            Procedural::Scales => scales(x, y),
            Procedural::GreekKey => greek_key(x, y),
            Procedural::CelticKnot => celtic_knot(x, y),
            Procedural::Floral => floral(x, y),
            Procedural::Hammered => hammered(x, y),
            Procedural::Waves => waves(x, y),
            Procedural::Diamonds => diamonds(x, y),
            Procedural::Beads => beads(x, y),
            Procedural::Feather => feather(x, y),
            Procedural::Bark => bark(x, y),
            Procedural::Nugget => nugget(x, y),
            Procedural::Rosette => rosette(x, y),
            Procedural::Barleycorn => barleycorn(x, y),
            Procedural::Moire => moire(x, y),
            Procedural::Starburst => starburst(x, y),
            Procedural::Florentine => florentine(x, y),
            Procedural::GuillocheWeave => guilloche_weave(x, y),
            Procedural::Voronoi => voronoi(x, y),
            Procedural::Trellis => trellis(x, y),
            Procedural::Honeycomb => honeycomb(x, y),
            Procedural::Pyramids => pyramids(x, y),
            Procedural::Argyle => argyle(x, y),
            Procedural::Lattice => lattice(x, y),
        }
    }
}

/// A builtin generator with knobs, carried in the design and re-baked on
/// load like drawn strokes and inscriptions. Every knob preserves the
/// pattern's seamlessness by construction: repeats are integer periods,
/// turns are quarter turns, and gamma/invert act on the value alone.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProcRecipe {
    /// Library name the raster lands under.
    pub name: String,
    pub kind: Procedural,
    /// Pattern periods per tile, 1..6.
    pub repeats: u32,
    /// Quarter turns, 0..3.
    pub quarter_turns: u32,
    /// Gamma on the value. Above 1 deepens, below 1 flattens.
    pub gamma: f64,
    pub invert: bool,
}

impl Default for ProcRecipe {
    fn default() -> Self {
        Self {
            name: "Recipe".into(),
            kind: Procedural::Waves,
            repeats: 1,
            quarter_turns: 0,
            gamma: 1.0,
            invert: false,
        }
    }
}

impl ProcRecipe {
    pub fn rasterize(&self, size: usize) -> Alpha {
        let n = size.clamp(16, 1024);
        let reps = self.repeats.clamp(1, 6) as f64;
        let turns = self.quarter_turns % 4;
        let g = self.gamma.clamp(0.3, 3.0);
        let mut raw = vec![0.0f64; n * n];
        let inv = 1.0 / n as f64;
        for j in 0..n {
            for i in 0..n {
                let (mut x, mut y) = (i as f64 * inv, j as f64 * inv);
                for _ in 0..turns {
                    (x, y) = (y, 1.0 - x);
                }
                let v = self.kind.at(frac(x * reps), frac(y * reps));
                raw[j * n + i] = if v.is_finite() { v } else { 0.0 };
            }
        }
        let mut data = rescale01(&raw);
        for v in &mut data {
            let mut t = (*v as f64).clamp(0.0, 1.0).powf(g);
            if self.invert {
                t = 1.0 - t;
            }
            *v = t as f32;
        }
        Alpha::new(self.name.clone(), n, n, data)
    }
}

// --- Pattern helpers -------------------------------------------------------

/// Fractional part, always 0..1.
#[inline]
fn frac(x: f64) -> f64 {
    x - x.floor()
}

/// Shortest signed delta on a unit-period circle, -0.5..0.5.
#[inline]
fn wrap1(d: f64) -> f64 {
    d - d.round()
}

/// Circular cross-section: 1 at the centre, 0 at |t| = 1.
#[inline]
fn dome(t: f64) -> f64 {
    (1.0 - t * t).max(0.0).sqrt()
}

/// Distance from a point to a line segment.
fn seg_dist(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let (vx, vy) = (bx - ax, by - ay);
    let (wx, wy) = (px - ax, py - ay);
    let len2 = vx * vx + vy * vy;
    let t = if len2 <= 1e-12 { 0.0 } else { ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0) };
    let (dx, dy) = (wx - t * vx, wy - t * vy);
    (dx * dx + dy * dy).sqrt()
}

/// Deterministic 0..1 hash of a lattice cell.
fn hash01(ix: i64, iy: i64, seed: u64) -> f64 {
    let mut h = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((ix as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93))
        .wrapping_add((iy as u64).wrapping_mul(0xA076_1D64_78BD_642F));
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 29;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 32;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// Value noise on a lattice that wraps after `px` by `py` cells. Returns 0..1.
fn value_noise(x: f64, y: f64, px: i64, py: i64, seed: u64) -> f64 {
    let (px, py) = (px.max(1), py.max(1));
    let fx = x * px as f64;
    let fy = y * py as f64;
    let (bx, by) = (fx.floor(), fy.floor());
    let tx = smoothstep(0.0, 1.0, fx - bx);
    let ty = smoothstep(0.0, 1.0, fy - by);
    let (ix, iy) = (bx as i64, by as i64);
    let at = |a: i64, b: i64| hash01(a.rem_euclid(px), b.rem_euclid(py), seed);
    let v00 = at(ix, iy);
    let v10 = at(ix + 1, iy);
    let v01 = at(ix, iy + 1);
    let v11 = at(ix + 1, iy + 1);
    let a = v00 + (v10 - v00) * tx;
    let b = v01 + (v11 - v01) * tx;
    a + (b - a) * ty
}

/// Octave sum of [`value_noise`], each octave doubling the lattice. Returns 0..1.
fn fbm(x: f64, y: f64, px: i64, py: i64, octaves: u32, seed: u64) -> f64 {
    let (mut px, mut py) = (px.max(1), py.max(1));
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut total = 0.0;
    for o in 0..octaves.max(1) {
        sum += amp * value_noise(x, y, px, py, seed.wrapping_add(o as u64 * 977));
        total += amp;
        amp *= 0.5;
        px *= 2;
        py *= 2;
    }
    sum / total.max(1e-9)
}

/// Rescale a raw pattern buffer to span 0..1.
fn rescale01(raw: &[f64]) -> Vec<f32> {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &v in raw {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    if !lo.is_finite() || !hi.is_finite() || (hi - lo) < 1e-9 {
        return raw.iter().map(|&v| v.clamp(0.0, 1.0) as f32).collect();
    }
    let inv = 1.0 / (hi - lo);
    raw.iter().map(|&v| (((v - lo) * inv).clamp(0.0, 1.0)) as f32).collect()
}

// --- Pattern generators ----------------------------------------------------

/// Twisted round cords running horizontally, ridges spiralling around each.
fn rope(x: f64, y: f64) -> f64 {
    let cords = 3.0;
    let twists = 6.0;
    let t = frac(y * cords);
    let s = (2.0 * t - 1.0) / 0.88;
    if s.abs() >= 1.0 {
        return 0.0;
    }
    let body = dome(s);
    let ridge = 0.5 + 0.5 * (TAU * (twists * x + t)).cos();
    body * (0.30 + 0.70 * ridge)
}

/// Three strands plaited, the front strand set by its phase.
fn braid(x: f64, y: f64) -> f64 {
    let k = 2.0;
    let half = 0.25;
    let mut h: f64 = 0.0;
    for s in 0..3 {
        let ph = TAU * (k * x + s as f64 / 3.0);
        let cy = 0.5 + 0.27 * ph.sin();
        let d = wrap1(y - cy).abs() / half;
        if d < 1.0 {
            let front = 0.5 + 0.5 * ph.cos();
            h = h.max(dome(d) * (0.40 + 0.60 * front));
        }
    }
    h
}

/// Plain weave: the strap on top alternates by cell parity.
fn basketweave(x: f64, y: f64) -> f64 {
    let n = 4.0;
    let strap = |t: f64| (PI * t).sin().max(0.0).powf(0.65);
    let hor = strap(frac(y * n));
    let ver = strap(frac(x * n));
    let ix = (x * n).floor() as i64;
    let iy = (y * n).floor() as i64;
    let (over, under) = if (ix + iy).rem_euclid(2) == 0 { (hor, ver) } else { (ver, hor) };
    over.max(under * 0.55)
}

/// Stacked V bands with flat crowns.
fn chevron(x: f64, y: f64) -> f64 {
    let nx = 2.0;
    let ny = 6.0;
    let tri = (2.0 * (frac(x * nx) - 0.5)).abs();
    let s = frac(y * ny + tri);
    let b = (2.0 * s - 1.0).abs();
    let band = 1.0 - smoothstep(0.45, 0.95, b);
    band * (0.80 + 0.20 * dome(b))
}

/// Diagonal bars whose slope flips every strip, offset half a bar per strip.
fn herringbone(x: f64, y: f64) -> f64 {
    let ns = 4.0;
    let kr = 8.0;
    let strip = (x * ns).floor() as i64;
    let q = if strip.rem_euclid(2) == 0 { (x + y) * kr } else { (x - y) * kr + 0.5 };
    let b = (2.0 * frac(q) - 1.0).abs();
    let bar = 1.0 - smoothstep(0.55, 1.0, b);
    let joint = (frac(x * ns) - 0.5).abs() * 2.0;
    bar * (0.82 + 0.18 * dome(b)) * (1.0 - 0.7 * smoothstep(0.86, 1.0, joint))
}

/// Domed scales on a staggered lattice. Each scale hangs two rows down, so the
/// nearest row in front hides all but a crescent of the row behind.
fn scales(x: f64, y: f64) -> f64 {
    let nx = 6.0;
    let ny = 8.0;
    let hx = 0.66 / nx;
    let hy = 1.05 / ny;
    let row = (y * ny).floor() as i64;
    for dj in [0i64, -1, -2] {
        let j = row + dj;
        let stagger = 0.5 * (j.rem_euclid(2) as f64);
        let cy = (j as f64 + 1.05) / ny;
        let base = (x * nx - stagger).round() as i64;
        let mut near = f64::MAX;
        for di in [-1i64, 0, 1] {
            let cx = (base + di) as f64 / nx + stagger / nx;
            let dx = wrap1(x - cx) / hx;
            let dy = wrap1(y - cy) / hy;
            near = near.min((dx * dx + dy * dy).sqrt());
        }
        if near < 1.0 {
            return (0.45 + 0.55 * dome(near)) * (1.0 - smoothstep(0.94, 1.0, near));
        }
    }
    0.0
}

/// The meander fret: a squared spiral rising off a continuous baseline.
fn greek_key(x: f64, y: f64) -> f64 {
    const G: f64 = 8.0;
    const SEGS: [(f64, f64, f64, f64); 7] = [
        (0.0, 1.0, 8.0, 1.0),
        (1.0, 1.0, 1.0, 7.0),
        (1.0, 7.0, 7.0, 7.0),
        (7.0, 7.0, 7.0, 3.0),
        (7.0, 3.0, 3.0, 3.0),
        (3.0, 3.0, 3.0, 5.0),
        (3.0, 5.0, 5.0, 5.0),
    ];
    let n = 3.0;
    let gx = frac(x * n) * G;
    let gy = frac(y * n) * G;
    let mut d = f64::MAX;
    for oy in -1..=1 {
        for ox in -1..=1 {
            let px = gx - ox as f64 * G;
            let py = gy - oy as f64 * G;
            for s in SEGS {
                d = d.min(seg_dist(px, py, s.0, s.1, s.2, s.3));
            }
        }
    }
    let bar = 1.0 - smoothstep(0.32, 0.52, d);
    bar * (0.74 + 0.26 * dome((d / 0.5).min(1.0)))
}

/// Two diagonal band families interlacing, the under band ducking at crossings.
fn celtic_knot(x: f64, y: f64) -> f64 {
    let k = 4.0;
    let a = (x + y) * k;
    let b = (x - y) * k;
    let band = |t: f64| {
        let d = (2.0 * frac(t) - 1.0).abs();
        let s = (d / 0.8).min(1.0);
        dome(s) * (0.55 + 0.45 * dome(s))
    };
    let ha = band(a);
    let hb = band(b);
    let a_over = (a.floor() as i64 + b.floor() as i64).rem_euclid(2) == 0;
    let (top, bot) = if a_over { (ha, hb) } else { (hb, ha) };
    top.max(bot * (1.0 - 0.92 * smoothstep(0.1, 0.75, top)))
}

/// Rosettes with petal lobes strung on a wavy vine.
fn floral(x: f64, y: f64) -> f64 {
    let n = 2.0;
    let petals = 6.0;
    let r0 = 0.24;
    let mut h: f64 = 0.0;
    for j in 0..2i64 {
        let stagger = 0.5 * (j.rem_euclid(2) as f64);
        let row = (j as f64 + 0.5) / n;
        let vine = row + 0.14 * (TAU * (n * x - 0.5 - stagger)).sin();
        let dv = wrap1(y - vine).abs() / 0.030;
        if dv < 1.0 {
            h = h.max(0.45 * dome(dv));
        }
        for i in 0..2i64 {
            let cx = (i as f64 + 0.5 + stagger) / n;
            let dx = wrap1(x - cx);
            let dy = wrap1(y - row);
            let r = (dx * dx + dy * dy).sqrt();
            let rr = r0 * (0.60 + 0.40 * (petals * dy.atan2(dx)).cos());
            if r < rr {
                let t = r / rr.max(1e-9);
                let petal = (0.25 + 0.60 * dome(t)) * (1.0 - smoothstep(0.86, 1.0, t));
                let pistil = dome((r / (0.30 * r0)).min(1.0));
                h = h.max(petal.max(pistil));
            }
        }
    }
    h
}

/// Overlapping planishing dimples on a jittered lattice.
fn hammered(x: f64, y: f64) -> f64 {
    let n = 5i64;
    let r = 0.95 / n as f64;
    let ix = (x * n as f64).floor() as i64;
    let iy = (y * n as f64).floor() as i64;
    let mut h: f64 = 1.0;
    for oy in -1..=1 {
        for ox in -1..=1 {
            let (cxi, cyi) = (ix + ox, iy + oy);
            let (mx, my) = (cxi.rem_euclid(n), cyi.rem_euclid(n));
            let jx = hash01(mx, my, 11);
            let jy = hash01(mx, my, 23);
            let cx = (cxi as f64 + 0.15 + 0.70 * jx) / n as f64;
            let cy = (cyi as f64 + 0.15 + 0.70 * jy) / n as f64;
            let dx = wrap1(x - cx);
            let dy = wrap1(y - cy);
            let d = (dx * dx + dy * dy).sqrt() / r;
            if d < 1.0 {
                let depth = 0.45 + 0.35 * hash01(mx, my, 37);
                let bowl = 1.0 - depth * (1.0 - d * d).powi(2);
                h = h.min(bowl);
            }
        }
    }
    h
}

/// Rolling wave bands, each rising slowly and breaking over its crest.
fn waves(x: f64, y: f64) -> f64 {
    let ny = 4.0;
    let warp = 0.35 * (TAU * x).sin() + 0.18 * (TAU * 2.0 * x + 1.7).sin();
    let t = frac(y * ny + warp);
    let body = smoothstep(0.0, 0.72, t) * (1.0 - smoothstep(0.72, 1.0, t));
    let crest = (-((t - 0.66) / 0.08).powi(2)).exp();
    body.powf(0.8) * (0.86 + 0.14 * (TAU * 3.0 * x).cos()) + 0.30 * crest
}

/// Crosshatched pyramid facets, a knurl.
fn diamonds(x: f64, y: f64) -> f64 {
    let k = 7.0;
    let ta = 1.0 - (2.0 * frac((x + y) * k) - 1.0).abs();
    let tb = 1.0 - (2.0 * frac((x - y) * k) - 1.0).abs();
    ta.min(tb).powf(0.5)
}

/// Granulation: a staggered lattice of hemispheres.
fn beads(x: f64, y: f64) -> f64 {
    let nx = 8.0;
    let ny = 8.0;
    let r = 0.60 / nx;
    let row = (y * ny - 0.5).round() as i64;
    let mut h: f64 = 0.0;
    for dj in [-1i64, 0, 1] {
        let j = row + dj;
        let stagger = 0.5 * (j.rem_euclid(2) as f64);
        let cy = (j as f64 + 0.5) / ny;
        let base = (x * nx - 0.5 - stagger).round() as i64;
        for di in [-1i64, 0, 1] {
            let cx = (base + di) as f64 / nx + (0.5 + stagger) / nx;
            let dx = wrap1(x - cx);
            let dy = wrap1(y - cy);
            let t = (dx * dx + dy * dy).sqrt() / r;
            if t < 1.0 {
                h = h.max(dome(t));
            }
        }
    }
    h
}

/// Barbs swept off a central quill, meeting tip to tip at the tile edge.
fn feather(x: f64, y: f64) -> f64 {
    let nb = 12i64;
    let s = wrap1(x - 0.5).abs();
    let p = (y + 1.7 * s) * nb as f64;
    let barb = dome(2.0 * frac(p) - 1.0).powf(0.7);
    // Per-barb length: the index is constant along a barb, so the vane frays.
    let tip = 0.34 + 0.13 * hash01((p.floor() as i64).rem_euclid(nb), 0, 5);
    let vane = smoothstep(0.0, 0.09, s) * (1.0 - smoothstep(tip - 0.13, tip, s));
    let quill = dome((s / 0.055).min(1.0));
    quill.max(vane * (0.22 + 0.78 * barb))
}

/// Irregular vertical furrows, the striation warped by wrapping noise.
fn bark(x: f64, y: f64) -> f64 {
    let w = fbm(x, y, 6, 2, 3, 11) - 0.5;
    let w2 = fbm(x, y, 3, 1, 2, 29) - 0.5;
    let grain = fbm(x, y, 24, 3, 2, 47) - 0.5;
    let s = x * 9.0 + 0.85 * w + 0.40 * w2;
    let ridge = (PI * frac(s)).sin().abs().powf(0.55);
    let split = 0.40 + 0.60 * smoothstep(-0.18, 0.22, w2);
    (ridge * split + 0.10 * grain).max(0.0)
}

/// Molten lumps with reticulated ridges.
fn nugget(x: f64, y: f64) -> f64 {
    // Warping the lookup with periodic noise hides the sampling lattice.
    let wx = x + 0.10 * (fbm(x, y, 3, 3, 2, 301) - 0.5);
    let wy = y + 0.10 * (fbm(x, y, 3, 3, 2, 401) - 0.5);
    let lump = fbm(wx, wy, 5, 5, 4, 101);
    let fine = fbm(wx, wy, 11, 11, 2, 202);
    let surface = smoothstep(0.20, 0.84, lump);
    let molten = (1.0 - (2.0 * fine - 1.0).abs()).powf(1.4);
    0.60 * surface + 0.28 * molten * (0.45 + 0.55 * surface) + 0.12 * lump
}

/// Cellular relief: a jittered site per lattice cell, cells raised as domes
/// and their shared boundaries recessed to a groove — the castable read of
/// CrossGems' Auto_Voronoi (3x3 sites per tile). Seamless because the site
/// hash wraps at the lattice period, so the F1/F2 field is periodic in both
/// axes. Cell interiors face the pull and the grooves are recesses, so on a
/// side face the whole field releases.
fn voronoi(x: f64, y: f64) -> f64 {
    let k: i64 = 3;
    let (fx, fy) = (x * k as f64, y * k as f64);
    let (bx, by) = (fx.floor() as i64, fy.floor() as i64);
    let (mut f1, mut f2) = (f64::MAX, f64::MAX);
    for dj in -1..=1 {
        for di in -1..=1 {
            let (ci, cj) = (bx + di, by + dj);
            // Site jittered inside its cell; the hash wraps at k so the tile
            // repeats. Kept off the cell walls (0.15..0.85) so no two sites
            // collide across a seam.
            let sx = ci as f64 + 0.15 + 0.70 * hash01(ci.rem_euclid(k), cj.rem_euclid(k), 5501);
            let sy = cj as f64 + 0.15 + 0.70 * hash01(ci.rem_euclid(k), cj.rem_euclid(k), 5507);
            let d = ((fx - sx).powi(2) + (fy - sy).powi(2)).sqrt();
            if d < f1 {
                f2 = f1;
                f1 = d;
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    // Boundary bevel from the F2-F1 gap; a gentle per-cell dome inside it.
    let wall = smoothstep(0.0, 0.16, f2 - f1);
    let mound = 0.55 + 0.45 * dome((f1 * 1.4).min(1.0));
    wall * mound
}

/// An open diagonal lattice of round wires — two families crossing at right
/// angles on the bias, the castable read of the Wire_Pattern inlay (4 wires
/// each way per tile). Round cross-sections (no vertical wall at a wire's own
/// edge), raised over recessed gaps, so a side face releases; the families
/// merge at the crossings into one grille.
fn trellis(x: f64, y: f64) -> f64 {
    let k = 4.0;
    let hw = 0.22;
    let wire = |phase: f64| {
        let d = wrap1(phase).abs();
        if d < hw {
            dome(d / hw)
        } else {
            0.0
        }
    };
    let a = wire((x + y) * k);
    let b = wire((x - y) * k);
    a.max(b)
}

/// Regular comb: raised hexagonal cell walls over recessed cells, from a
/// staggered lattice of sites (even rows and columns so the half-cell
/// stagger wraps). Walls sit where the two nearest sites tie, so the ridge
/// network is the honeycomb. Side-face feature.
fn honeycomb(x: f64, y: f64) -> f64 {
    let (cols, rows) = (4i64, 4i64);
    let (fx, fy) = (x * cols as f64, y * rows as f64);
    let (bx, by) = (fx.floor() as i64, fy.floor() as i64);
    let (mut f1, mut f2) = (f64::MAX, f64::MAX);
    for dj in -1..=1 {
        for di in -1..=1 {
            let (ci, cj) = (bx + di, by + dj);
            let stagger = 0.5 * (cj.rem_euclid(2) as f64);
            let d = ((fx - (ci as f64 + 0.5 + stagger)).powi(2)
                + ((fy - (cj as f64 + 0.5)) * 1.15).powi(2))
            .sqrt();
            if d < f1 {
                f2 = f1;
                f1 = d;
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    1.0 - smoothstep(0.0, 0.24, f2 - f1)
}

/// A grid of four-sided pyramid studs — a crisp faceted hobnail. The
/// Chebyshev ramp gives straight facets meeting at a point; seamless in both
/// axes, each stud a convex mound.
fn pyramids(x: f64, y: f64) -> f64 {
    let k = 5.0;
    let fx = frac(x * k);
    let fy = frac(y * k);
    (1.0 - (2.0 * fx - 1.0).abs().max((2.0 * fy - 1.0).abs())).max(0.0)
}

/// Argyle: a thin round-wire diamond lattice on the bias with a raised dot in
/// each diamond. Integer diagonal periods, so it tiles; ridges and dots are
/// round-profiled, castable on a side face.
fn argyle(x: f64, y: f64) -> f64 {
    let k = 4.0;
    let hw = 0.13;
    let ridge = |p: f64| {
        let d = wrap1(p).abs();
        if d < hw {
            0.8 * dome(d / hw)
        } else {
            0.0
        }
    };
    let lines = ridge((x + y) * k).max(ridge((x - y) * k));
    let (da, db) = (wrap1((x + y) * k - 0.5), wrap1((x - y) * k - 0.5));
    let dist = (da * da + db * db).sqrt();
    let dr = 0.22;
    let dot = if dist < dr { dome(dist / dr) } else { 0.0 };
    lines.max(dot)
}

/// An orthogonal open grille: flat-topped bars along the grid lines over
/// recessed square wells. Distinct from the diagonal round-wire Trellis;
/// bars stand square to the pull on a side face, the wells are recesses.
fn lattice(x: f64, y: f64) -> f64 {
    let k = 4.0;
    let hw = 0.28;
    let bar = |p: f64| {
        let d = wrap1(p).abs();
        1.0 - smoothstep(hw * 0.55, hw, d)
    };
    bar(x * k).max(bar(y * k))
}

// --- Engine turning --------------------------------------------------------

/// Rose-engine medallion: concentric grooves wobbled by an angular lobe count,
/// one rosette per tile meeting its neighbours at the seams.
fn rosette(x: f64, y: f64) -> f64 {
    let dx = wrap1(x - 0.5);
    let dy = wrap1(y - 0.5);
    let r = (dx * dx + dy * dy).sqrt();
    let a = dy.atan2(dx);
    let rings = 9.0;
    let lobes = 12.0;
    let groove = 0.5 + 0.5 * (TAU * rings * r + 1.7 * (lobes * a).sin()).cos();
    let hub = 1.0 - smoothstep(0.02, 0.10, r);
    groove.powf(1.3).max(hub)
}

/// Staggered rows of pointed grains.
fn barleycorn(x: f64, y: f64) -> f64 {
    let nx = 7.0;
    let ny = 11.0;
    let row = (y * ny).floor() as i64;
    let sx = x * nx + 0.5 * row.rem_euclid(2) as f64;
    let cx = (frac(sx) - 0.5) * 2.0;
    let cy = (frac(y * ny) - 0.5) * 2.0;
    // Pointed along the row: the x term rises slower near the centre and
    // reaches 1 sooner at the tips than a circle would.
    let d = cx.abs().powf(1.5) + (cy * 1.15).powi(2);
    if d >= 1.0 { 0.0 } else { (1.0 - d).sqrt() }
}

/// Two integer gratings a few cycles apart; their product beats slowly.
fn moire(x: f64, y: f64) -> f64 {
    let g1 = (TAU * (14.0 * x + y)).sin();
    let g2 = (TAU * (12.0 * x - y)).sin();
    let beat = g1 * g2;
    0.5 + 0.5 * beat
}

/// Rays from the tile centre, ridge-sharpened, over faint concentric rings.
fn starburst(x: f64, y: f64) -> f64 {
    let dx = wrap1(x - 0.5);
    let dy = wrap1(y - 0.5);
    let r = (dx * dx + dy * dy).sqrt();
    let a = dy.atan2(dx);
    let rays = 24.0;
    let ray = (0.5 + 0.5 * (rays * a).cos()).powf(2.2);
    let ring = 0.82 + 0.18 * (TAU * 15.0 * r).cos();
    let hub = 1.0 - smoothstep(0.015, 0.06, r);
    (ray * ring).max(hub)
}

/// Fine cross-hatch cut into a plateau, each family gently wobbled. The lines
/// are narrow against their pitch, so the surface stays a plateau carrying
/// engraving rather than a woven check.
fn florentine(x: f64, y: f64) -> f64 {
    let n = 24.0;
    let wob = 0.04;
    let g1 = (2.0 * frac(n * x + wob * (TAU * 3.0 * y).sin()) - 1.0).abs();
    let g2 = (2.0 * frac(n * y + wob * (TAU * 3.0 * x).sin()) - 1.0).abs();
    let cut1 = 1.0 - smoothstep(0.08, 0.22, g1);
    let cut2 = 1.0 - smoothstep(0.08, 0.22, g2);
    1.0 - 0.60 * cut1.max(0.55 * cut2)
}

/// Two sine cords per band, half a cycle apart, crossing in the lens-shaped
/// cells of a woven watch-dial guilloche. Both families share every row, so
/// the tile stays periodic in `y` at the band pitch.
fn guilloche_weave(x: f64, y: f64) -> f64 {
    let rows = 4.0;
    let k = 4.0;
    let amp = 0.58 / rows;
    let half = 0.24 / rows;
    let row = (y * rows).floor() as i64;
    let mut h: f64 = 0.0;
    for dj in -1i64..=1 {
        let j = (row + dj) as f64;
        for ph in [0.0, 0.5] {
            let phase = TAU * (k * x + ph);
            let cy = (j + 0.5) / rows + amp * phase.sin();
            let d = (y - cy).abs() / half;
            if d < 1.0 {
                // The rising cord reads as the one on top.
                let front = 0.5 + 0.5 * phase.cos();
                h = h.max(dome(d) * (0.62 + 0.38 * front));
            }
        }
    }
    h
}

/// Named collection of alphas. Layers reference entries by name so a saved
/// design survives the library being reordered.
#[derive(Clone, Debug, Default)]
pub struct AlphaLibrary {
    entries: Vec<Alpha>,
    index: HashMap<String, usize>,
}

impl AlphaLibrary {
    /// Every [`Procedural`] pattern, rendered at the default resolution.
    pub fn builtin() -> Self {
        let mut lib = Self::default();
        for p in Procedural::ALL {
            lib.insert(p.generate(BUILTIN_SIZE));
        }
        lib
    }

    /// Most entries a library will hold.
    ///
    /// `drawn`, `texts`, `svgs` and `recipes` are `Vec`s read straight out of
    /// a design file, and every one of them rasterizes into here on load. The
    /// per-alpha size is already capped; the *count* was not, so a file with
    /// ten thousand one-glyph inscriptions was a memory bomb with no loop to
    /// blame. The 67 GB lesson, applied to the other door.
    pub const MAX_ENTRIES: usize = 4096;

    pub fn insert(&mut self, alpha: Alpha) {
        match self.index.get(&alpha.name).copied() {
            Some(i) => self.entries[i] = alpha,
            None => {
                if self.entries.len() >= Self::MAX_ENTRIES {
                    log::warn!(
                        "alpha library is full at {} entries; dropping {:?}",
                        Self::MAX_ENTRIES,
                        alpha.name
                    );
                    return;
                }
                self.index.insert(alpha.name.clone(), self.entries.len());
                self.entries.push(alpha);
            }
        }
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let Some(i) = self.index.get(name).copied() else {
            return false;
        };
        self.entries.remove(i);
        self.index = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, a)| (a.name.clone(), i))
            .collect();
        true
    }

    pub fn get(&self, name: &str) -> Option<&Alpha> {
        self.index.get(name).and_then(|&i| self.entries.get(i))
    }

    pub fn get_index(&self, i: usize) -> Option<&Alpha> {
        self.entries.get(i)
    }

    pub fn position(&self, name: &str) -> Option<usize> {
        self.index.get(name).copied()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Alpha> {
        self.entries.iter()
    }

    pub fn names(&self) -> Vec<String> {
        self.entries.iter().map(|a| a.name.clone()).collect()
    }

    /// Load every image in a directory into the library. Returns how many were
    /// added. A missing directory is not an error.
    pub fn load_dir(&mut self, dir: impl AsRef<Path>) -> anyhow::Result<usize> {
        let dir = dir.as_ref();
        if !dir.is_dir() {
            return Ok(0);
        }
        let mut paths: Vec<PathBuf> = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp") {
                paths.push(path);
            }
        }
        paths.sort();
        let mut added = 0;
        let mut skipped = 0;
        for path in paths {
            if self.entries.len() >= MAX_LIBRARY_ENTRIES {
                skipped += 1;
                continue;
            }
            match Alpha::load(&path) {
                Ok(a) => {
                    self.insert(a);
                    added += 1;
                }
                Err(e) => log::warn!("skipping alpha {}: {e}", path.display()),
            }
        }
        if skipped > 0 {
            log::warn!("library full at {MAX_LIBRARY_ENTRIES} entries, skipped {skipped} file(s)");
        }
        Ok(added)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_recipe_stays_seamless_under_every_knob() {
        let r = super::ProcRecipe {
            name: "test".into(),
            kind: super::Procedural::Waves,
            repeats: 3,
            quarter_turns: 1,
            gamma: 1.8,
            invert: true,
        };
        let a = r.rasterize(128);
        let n = a.width;
        // Seamless means the step across the tile boundary is no bigger than
        // the pattern's own steepest interior step — the seam pair is just
        // another adjacent pair of the periodic function.
        let mut interior = 0.0f32;
        for j in 0..n {
            for i in 0..n - 1 {
                interior = interior.max((a.data[j * n + i + 1] - a.data[j * n + i]).abs());
            }
        }
        for j in 0..n {
            let d = (a.data[j * n] - a.data[j * n + n - 1]).abs();
            assert!(d <= interior * 1.5 + 1e-3, "x seam tears at row {j}: {d} vs {interior}");
        }
        for i in 0..n {
            let d = (a.data[i] - a.data[(n - 1) * n + i]).abs();
            assert!(d <= interior * 1.5 + 1e-3, "y seam tears at col {i}: {d} vs {interior}");
        }
        // The knobs do something: a plain bake differs.
        let plain = super::ProcRecipe {
            repeats: 1,
            quarter_turns: 0,
            gamma: 1.0,
            invert: false,
            ..r.clone()
        }
        .rasterize(128);
        let diff: f32 =
            a.data.iter().zip(&plain.data).map(|(x, y)| (x - y).abs()).sum::<f32>() / (n * n) as f32;
        assert!(diff > 0.05, "knobs changed nothing: {diff}");
    }

    /// Every builtin pattern must tile seamlessly — the seam step no larger
    /// than the pattern's own steepest interior step. Voronoi and Trellis
    /// join by construction (site hash wraps at the lattice period; the wire
    /// phases are integer-period), but the whole family is pinned here.
    #[test]
    fn every_pattern_tiles_seamlessly() {
        let n = 128usize;
        for &p in super::Procedural::ALL {
            let a = p.generate(n);
            let mut interior = 0.0f32;
            for j in 0..n {
                for i in 0..n - 1 {
                    interior = interior.max((a.data[j * n + i + 1] - a.data[j * n + i]).abs());
                }
            }
            let tol = interior * 1.5 + 1e-3;
            for j in 0..n {
                let d = (a.data[j * n] - a.data[j * n + n - 1]).abs();
                assert!(d <= tol, "{} x seam at row {j}: {d} vs interior {interior}", p.label());
            }
            for i in 0..n {
                let d = (a.data[i] - a.data[(n - 1) * n + i]).abs();
                assert!(d <= tol, "{} y seam at col {i}: {d} vs interior {interior}", p.label());
            }
        }
    }

    #[test]
    fn the_signed_distance_field_measures_a_disk() {
        let (w, h, r) = (64usize, 64usize, 20.0f64);
        let mut data = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let d = ((x as f64 - 32.0).powi(2) + (y as f64 - 32.0).powi(2)).sqrt();
                if d <= r {
                    data[y * w + x] = 1.0;
                }
            }
        }
        let a = super::Alpha::new("disk", w, h, data);
        let sdf = a.signed_distance_px();
        assert_eq!(sdf.name, "disk##sdf");
        let at = |x: usize, y: usize| sdf.data[y * w + x] as f64;
        // Centre sits a radius inside; the corner sits well outside.
        assert!((at(32, 32) - r).abs() < 1.6, "centre {}", at(32, 32));
        assert!(at(2, 32) < -8.0, "far ground {}", at(2, 32));
        // Distance changes one pixel per pixel along a radius.
        let step = at(32 - 10, 32) - at(32 - 14, 32);
        assert!((step - 4.0).abs() < 0.8, "gradient step {step}");
    }

    use super::*;

    #[test]
    fn from_bytes_decodes_the_same_picture_as_from_disk() {
        let dir = std::env::temp_dir().join(format!("ringdesign-frombytes-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let png = dir.join("ramp.png");
        image::save_buffer(&png, &[0u8, 85, 170, 255], 2, 2, image::ExtendedColorType::L8).unwrap();

        let from_disk = Alpha::load(&png).expect("load");
        let bytes = std::fs::read(&png).unwrap();
        let from_mem = Alpha::from_bytes("ramp", &bytes).expect("from_bytes");

        assert_eq!((from_disk.width, from_disk.height), (from_mem.width, from_mem.height));
        assert_eq!(from_disk.data, from_mem.data);
        assert_eq!(from_mem.name, "ramp");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn from_bytes_rejects_something_that_is_not_an_image() {
        assert!(Alpha::from_bytes("junk", b"not a png").is_err());
    }

    fn ramp(w: usize, h: usize) -> Alpha {
        let mut data = Vec::with_capacity(w * h);
        for _ in 0..h {
            for i in 0..w {
                data.push(i as f32 / (w - 1) as f32);
            }
        }
        Alpha::new("ramp", w, h, data)
    }

    /// Largest step between horizontally adjacent columns, and the step across
    /// the wrap seam.
    fn column_steps(a: &Alpha) -> (f64, f64) {
        let (mut interior, mut seam) = (0.0f64, 0.0f64);
        for j in 0..a.height {
            let row = j * a.width;
            for i in 0..a.width - 1 {
                interior = interior.max((a.data[row + i + 1] - a.data[row + i]).abs() as f64);
            }
            seam = seam.max((a.data[row] - a.data[row + a.width - 1]).abs() as f64);
        }
        (interior, seam)
    }

    fn row_steps(a: &Alpha) -> (f64, f64) {
        let (mut interior, mut seam) = (0.0f64, 0.0f64);
        for i in 0..a.width {
            for j in 0..a.height - 1 {
                let d = (a.data[(j + 1) * a.width + i] - a.data[j * a.width + i]).abs() as f64;
                interior = interior.max(d);
            }
            let d = (a.data[i] - a.data[(a.height - 1) * a.width + i]).abs() as f64;
            seam = seam.max(d);
        }
        (interior, seam)
    }

    #[test]
    fn bilinear_interpolates_between_texels() {
        let a = Alpha::new("t", 2, 2, vec![0.0, 1.0, 1.0, 0.0]);
        assert_eq!(a.sample(0.0, 0.0), 0.0);
        assert_eq!(a.sample(1.0, 0.0), 1.0);
        assert_eq!(a.sample(0.0, 1.0), 1.0);
        assert_eq!(a.sample(1.0, 1.0), 0.0);
        assert!((a.sample(0.5, 0.0) - 0.5).abs() < 1e-6);
        assert!((a.sample(0.5, 0.5) - 0.5).abs() < 1e-6);
        assert!((a.sample(0.25, 0.0) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn sample_clamps_and_sample_wrapped_repeats() {
        let a = Alpha::new("t", 2, 2, vec![0.0, 1.0, 1.0, 0.0]);
        assert_eq!(a.sample(-3.0, 0.0), a.sample(0.0, 0.0));
        assert_eq!(a.sample(4.0, 0.5), a.sample(1.0, 0.5));
        assert_eq!(a.sample(f64::NAN, f64::NAN), a.sample(0.0, 0.0));

        for k in [-2.0, -1.0, 1.0, 3.0] {
            assert_eq!(a.sample_wrapped(0.3 + k, 0.7), a.sample_wrapped(0.3, 0.7));
            assert_eq!(a.sample_wrapped(0.3, 0.7 + k), a.sample_wrapped(0.3, 0.7));
        }
        // 0.5 lands exactly on the second texel; 0.75 is halfway back to the first.
        assert_eq!(a.sample_wrapped(0.5, 0.0), 1.0);
        assert!((a.sample_wrapped(0.75, 0.0) - 0.5).abs() < 1e-6);
        // Clamped and wrapped disagree past the last texel: one holds, one folds.
        assert_eq!(a.sample(1.0, 0.0), 1.0);
        assert_eq!(a.sample_wrapped(1.0, 0.0), 0.0);
    }

    #[test]
    fn empty_alpha_samples_to_zero() {
        let a = Alpha::default();
        assert_eq!(a.sample(0.5, 0.5), 0.0);
        assert_eq!(a.sample_wrapped(0.5, 0.5), 0.0);
        assert!(a.rgba8().is_empty());
    }

    #[test]
    fn draft_limiting_caps_every_wall_and_stays_seamless() {
        // A hard-edged checker: vertical cliffs everywhere.
        let n = 64usize;
        let data: Vec<f32> = (0..n * n)
            .map(|i| if ((i % n) / 16 + (i / n) / 16) % 2 == 0 { 1.0 } else { 0.0 })
            .collect();
        let a = Alpha::new("checker", n, n, data);

        // 0.5 mm cell pitch per 16 texels, 0.5 mm relief, 55 degree walls.
        let mm_per_px = (0.5 / 16.0, 0.5 / 16.0);
        let relief = 0.5;
        let b = a.draft_limited(55.0, mm_per_px, relief);
        let slope_px = (55f64.to_radians().tan() * mm_per_px.0 / relief) as f32;

        let at = |img: &Alpha, x: usize, y: usize| img.data[(y % n) * n + (x % n)];
        let mut worst = 0.0f32;
        for y in 0..n {
            for x in 0..n {
                // Wrapped neighbours, so the seam is checked too.
                worst = worst.max((at(&b, x, y) - at(&b, x + 1, y)).abs());
                worst = worst.max((at(&b, x, y) - at(&b, x, y + 1)).abs());
                assert!(at(&b, x, y) <= at(&a, x, y) + 1e-6, "opening only removes");
            }
        }
        assert!(
            worst <= slope_px * 1.01,
            "a wall survived the bake: step {worst} vs allowed {slope_px}"
        );
        let ink: f32 = b.data.iter().sum();
        assert!(ink > 100.0, "the pattern should survive, not vanish: {ink}");
    }

    #[test]
    fn every_procedural_is_the_requested_size_and_spans_the_range() {
        for &p in Procedural::ALL {
            let a = p.generate(64);
            assert_eq!((a.width, a.height), (64, 64), "{}", p.label());
            assert_eq!(a.data.len(), 64 * 64, "{}", p.label());
            assert_eq!(a.name, p.label());
            let mut lo = f32::INFINITY;
            let mut hi = f32::NEG_INFINITY;
            for &v in &a.data {
                assert!(v.is_finite() && (0.0..=1.0).contains(&v), "{} out of range: {v}", p.label());
                lo = lo.min(v);
                hi = hi.max(v);
            }
            assert!(hi - lo > 0.5, "{} is nearly constant", p.label());
            assert!(lo < 0.02 && hi > 0.98, "{} does not span 0..1: {lo}..{hi}", p.label());
        }
    }

    #[test]
    fn every_procedural_tiles_without_a_seam() {
        for &p in Procedural::ALL {
            let a = p.generate(128);
            let (interior, seam) = column_steps(&a);
            assert!(
                seam <= interior * 1.5 + 1e-3,
                "{} seams horizontally: seam step {seam} vs interior {interior}",
                p.label()
            );
            let (interior, seam) = row_steps(&a);
            assert!(
                seam <= interior * 1.5 + 1e-3,
                "{} seams vertically: seam step {seam} vs interior {interior}",
                p.label()
            );
            // A sample one texel past the last column is the first column again.
            let step = 1.0 / a.width as f64;
            for j in 0..a.height {
                let y = j as f64 / a.height as f64;
                let past = a.sample_wrapped((a.width - 1) as f64 * step + step, y);
                assert_eq!(past, a.data[j * a.width], "{}", p.label());
            }
        }
    }

    #[test]
    fn procedurals_are_deterministic() {
        let a = Procedural::Nugget.generate(32);
        let b = Procedural::Nugget.generate(32);
        assert_eq!(a.data, b.data);
    }

    #[test]
    fn normalized_stretches_to_the_full_range() {
        let data: Vec<f32> = (0..16).map(|i| 0.4 + i as f32 * 0.01).collect();
        let a = Alpha::new("t", 4, 4, data);
        let n = a.normalized();
        let lo = n.data.iter().cloned().fold(f32::INFINITY, f32::min);
        let hi = n.data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(lo.abs() < 1e-6 && (hi - 1.0).abs() < 1e-6, "{lo}..{hi}");
        assert_eq!(n.width, a.width);
    }

    #[test]
    fn normalized_leaves_a_flat_image_alone() {
        let a = Alpha::new("t", 2, 2, vec![0.3; 4]);
        assert_eq!(a.normalized().data, a.data);
    }

    #[test]
    fn make_seamless_keeps_dimensions_and_kills_the_seam() {
        let a = ramp(32, 32);
        let (_, seam_before) = column_steps(&a);
        assert!(seam_before > 0.9);
        let s = a.make_seamless(0.25);
        assert_eq!((s.width, s.height), (a.width, a.height));
        assert_eq!(s.data.len(), a.data.len());
        let (_, seam_after) = column_steps(&s);
        assert!(seam_after < 1e-6, "seam survived: {seam_after}");
        // The untouched middle is unchanged.
        assert_eq!(s.data[16], a.data[16]);
    }

    #[test]
    fn make_seamless_clamps_its_blend() {
        let a = ramp(16, 16);
        assert_eq!(a.make_seamless(-1.0).data, a.data);
        let wide = a.make_seamless(4.0);
        assert_eq!((wide.width, wide.height), (16, 16));
    }

    #[test]
    fn rgba8_is_opaque_and_correctly_sized() {
        let a = Alpha::new("t", 2, 1, vec![0.0, 1.0]);
        let px = a.rgba8();
        assert_eq!(px.len(), a.width * a.height * 4);
        assert_eq!(&px[0..4], &[0, 0, 0, 255]);
        assert_eq!(&px[4..8], &[255, 255, 255, 255]);
    }

    #[test]
    fn builtin_contains_every_pattern() {
        let lib = AlphaLibrary::builtin();
        assert_eq!(lib.len(), Procedural::ALL.len());
        for &p in Procedural::ALL {
            let a = lib.get(p.label()).unwrap_or_else(|| panic!("missing {}", p.label()));
            assert_eq!((a.width, a.height), (BUILTIN_SIZE, BUILTIN_SIZE));
            assert_eq!(lib.get_index(lib.position(p.label()).unwrap()).unwrap().name, p.label());
        }
    }

    #[test]
    fn insert_replaces_by_name_and_remove_reindexes() {
        let mut lib = AlphaLibrary::default();
        lib.insert(Alpha::new("a", 1, 1, vec![0.0]));
        lib.insert(Alpha::new("b", 1, 1, vec![0.5]));
        lib.insert(Alpha::new("c", 1, 1, vec![1.0]));
        lib.insert(Alpha::new("b", 2, 1, vec![0.25, 0.75]));
        assert_eq!(lib.len(), 3);
        assert_eq!(lib.position("b"), Some(1));
        assert_eq!(lib.get("b").unwrap().width, 2);

        assert!(lib.remove("a"));
        assert!(!lib.remove("a"));
        assert_eq!(lib.names(), vec!["b".to_string(), "c".to_string()]);
        assert_eq!(lib.position("b"), Some(0));
        assert_eq!(lib.position("c"), Some(1));
        assert_eq!(lib.get("c").unwrap().data, vec![1.0]);
    }

    #[test]
    fn load_dir_of_a_missing_directory_adds_nothing() {
        let mut lib = AlphaLibrary::default();
        let n = lib.load_dir("/nonexistent/ringdesigner/alphas").unwrap();
        assert_eq!(n, 0);
        assert!(lib.is_empty());
    }

    #[test]
    fn load_takes_its_name_from_the_file_stem_and_load_dir_skips_other_files() {
        let dir = std::env::temp_dir().join(format!("ringdesign-alpha-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("swatch.png");
        image::save_buffer(&png, &[0u8, 85, 170, 255], 2, 2, image::ExtendedColorType::L8).unwrap();
        std::fs::write(dir.join("notes.txt"), b"not an image").unwrap();
        std::fs::write(dir.join("broken.png"), b"not a png either").unwrap();

        let a = Alpha::load(&png).unwrap();
        assert_eq!(a.name, "swatch");
        assert_eq!((a.width, a.height), (2, 2));
        assert!(a.data[0] < 1e-6 && (a.data[3] - 1.0).abs() < 1e-6, "{:?}", a.data);

        let mut lib = AlphaLibrary::default();
        assert_eq!(lib.load_dir(&dir).unwrap(), 1);
        assert!(lib.get("swatch").is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scratch_probe() {
        let fns: Vec<(&str, fn(f64, f64) -> f64)> = vec![
            ("Rope", rope),
            ("Braid", braid),
            ("Basketweave", basketweave),
            ("Chevron", chevron),
            ("Herringbone", herringbone),
            ("Scales", scales),
            ("GreekKey", greek_key),
            ("CelticKnot", celtic_knot),
            ("Floral", floral),
            ("Hammered", hammered),
            ("Waves", waves),
            ("Diamonds", diamonds),
            ("Beads", beads),
            ("Feather", feather),
            ("Bark", bark),
            ("Nugget", nugget),
        ];
        println!("== raw periodicity f(x,y) vs f(x+1,y) / f(x,y+1) ==");
        for (name, f) in &fns {
            let (mut mx, mut my) = (0.0f64, 0.0f64);
            for k in 0..257 {
                let t = k as f64 / 257.0;
                mx = mx.max((f(t, 0.3137) - f(t + 1.0, 0.3137)).abs());
                mx = mx.max((f(0.1237, t) - f(1.1237, t)).abs());
                my = my.max((f(0.3137, t) - f(0.3137, t + 1.0)).abs());
                my = my.max((f(t, 0.1237) - f(t, 1.1237)).abs());
            }
            println!("{name:12} dx={mx:.3e} dy={my:.3e}");
        }

        // Montage: each pattern tiled 2x2 at 128, four columns wide.
        let cell = 256usize;
        let cols = 4usize;
        let rows = Procedural::ALL.len().div_ceil(cols);
        let (mw, mh) = (cols * cell, rows * cell);
        let mut buf = vec![255u8; mw * mh];
        for (k, &p) in Procedural::ALL.iter().enumerate() {
            let a = p.generate(128);
            let (ox, oy) = ((k % cols) * cell, (k / cols) * cell);
            for y in 0..cell {
                for x in 0..cell {
                    let v = a.data[(y % 128) * 128 + (x % 128)];
                    let on_edge = x == 0 || y == 0 || x == cell - 1 || y == cell - 1;
                    buf[(oy + y) * mw + ox + x] =
                        if on_edge { 255 } else { (v.clamp(0.0, 1.0) * 255.0) as u8 };
                }
            }
        }
        // Only with `RD_ALPHA_SHEET=/some/dir`: the montage is for eyeballing, not asserting.
        let Ok(dir) = std::env::var("RD_ALPHA_SHEET") else { return };
        let _ = std::fs::create_dir_all(&dir);
        image::save_buffer(
            format!("{dir}/alphas.png"),
            &buf,
            mw as u32,
            mh as u32,
            image::ExtendedColorType::L8,
        )
        .unwrap();

        for &p in &[Procedural::Feather, Procedural::Braid, Procedural::Floral] {
            let a = p.generate(256);
            let n = 512usize;
            let mut b2 = vec![0u8; n * n];
            for y in 0..n {
                for x in 0..n {
                    b2[y * n + x] = (a.data[(y % 256) * 256 + (x % 256)] * 255.0) as u8;
                }
            }
            image::save_buffer(
                format!("{dir}/{}.png", p.label()),
                &b2,
                n as u32,
                n as u32,
                image::ExtendedColorType::L8,
            )
            .unwrap();
        }

        println!("== wrapped seam vs interior (generate 256) ==");
        for &p in Procedural::ALL {
            let a = p.generate(256);
            let e = 0.5 / 256.0;
            let (mut seam_x, mut int_x, mut seam_y, mut int_y) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            for k in 0..512 {
                let t = k as f64 / 512.0;
                seam_x = seam_x
                    .max((a.sample_wrapped(1.0 - e, t) as f64 - a.sample_wrapped(e, t) as f64).abs());
                seam_y = seam_y
                    .max((a.sample_wrapped(t, 1.0 - e) as f64 - a.sample_wrapped(t, e) as f64).abs());
                for m in 0..256 {
                    let s = m as f64 / 256.0;
                    int_x = int_x.max(
                        (a.sample_wrapped(s + e, t) as f64 - a.sample_wrapped(s - e, t) as f64).abs(),
                    );
                    int_y = int_y.max(
                        (a.sample_wrapped(t, s + e) as f64 - a.sample_wrapped(t, s - e) as f64).abs(),
                    );
                }
            }
            // Histogram spread.
            let mut bins = [0usize; 10];
            for &v in &a.data {
                bins[((v * 10.0) as usize).min(9)] += 1;
            }
            let mean: f64 = a.data.iter().map(|&v| v as f64).sum::<f64>() / a.data.len() as f64;
            let var: f64 =
                a.data.iter().map(|&v| (v as f64 - mean).powi(2)).sum::<f64>() / a.data.len() as f64;
            let mids = a.data.iter().filter(|&&v| (0.15..0.85).contains(&v)).count() as f64
                / a.data.len() as f64;
            println!(
                "{:12} seamx={seam_x:.4} intx={int_x:.4} seamy={seam_y:.4} inty={int_y:.4} mean={mean:.3} sd={:.3} mid%={:.3} bins={bins:?}",
                p.label(),
                var.sqrt(),
                mids
            );
        }
    }

    #[test]
    fn scratch_hostile() {
        // Struct-literal alphas whose data disagrees with the declared size.
        let liars = vec![
            Alpha { name: "short".into(), width: 4, height: 4, data: vec![0.5; 3] },
            Alpha { name: "long".into(), width: 2, height: 2, data: vec![0.5; 99] },
            Alpha { name: "zero_w".into(), width: 0, height: 4, data: vec![0.5; 4] },
            Alpha { name: "zero_h".into(), width: 4, height: 0, data: vec![0.5; 4] },
            Alpha { name: "nan".into(), width: 2, height: 2, data: vec![f32::NAN, 0.0, 1.0, -3.0] },
            Alpha { name: "one".into(), width: 1, height: 1, data: vec![0.25] },
            Alpha { name: "row".into(), width: 5, height: 1, data: vec![0.1; 5] },
            Alpha { name: "col".into(), width: 1, height: 5, data: vec![0.1; 5] },
        ];
        for a in &liars {
            for &x in &[-1e9, -0.5, 0.0, 0.3, 1.0, 1e9, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                for &y in &[-1e9, 0.0, 0.499_999, 1.0, f64::NAN, f64::INFINITY] {
                    let _ = a.sample(x, y);
                    let _ = a.sample_wrapped(x, y);
                }
            }
            let _ = a.rgba8();
            let _ = a.normalized();
            for &b in &[-1.0, 0.0, 0.1, 0.5, 9.0, f64::NAN, f64::INFINITY] {
                let s = a.make_seamless(b);
                assert_eq!((s.width, s.height), (a.width, a.height), "{}", a.name);
            }
            for &c in &[0.0, 1.0, 1e9, f64::NAN, -2.0] {
                for &bi in &[0.0, 0.5, f64::NAN, f64::INFINITY] {
                    let _ = a.shaped(0.5, c, bi, true);
                    let _ = a.shaped(f32::NAN, c, bi, false);
                }
            }
        }
        for n in [0usize, 1, 2, 3, 7] {
            for &p in Procedural::ALL {
                let a = p.generate(n);
                assert_eq!(a.data.len(), a.width * a.height, "{} at {n}", p.label());
                assert!(a.width >= 1 && a.height >= 1);
                assert!(a.data.iter().all(|v| v.is_finite()), "{} at {n}", p.label());
            }
        }
        // normalized / shaped must not leak NaN.
        let nanny = Alpha::new("n", 2, 2, vec![f32::NAN, 0.0, 1.0, 0.5]);
        println!("normalized(nan) = {:?}", nanny.normalized().data);
        println!("shaped(nan raw) = {}", nanny.shaped(f32::NAN, 1.0, 0.0, false));
        println!("shaped(nan bias) = {}", nanny.shaped(0.5, 1.0, f64::NAN, false));
        println!("sample(nan data) = {}", nanny.sample(0.1, 0.1));

        // make_seamless on both axes.
        let mut vdata = Vec::new();
        for j in 0..32 {
            for _ in 0..32 {
                vdata.push(j as f32 / 31.0);
            }
        }
        let v = Alpha::new("vramp", 32, 32, vdata);
        let (_, sb) = row_steps(&v);
        let s = v.make_seamless(0.25);
        let (ia, sa) = row_steps(&s);
        println!("vertical ramp: seam before {sb:.4} after {sa:.6} interior {ia:.4}");

        // rgba8 of an alpha whose data is short.
        let short = Alpha { name: "s".into(), width: 3, height: 3, data: vec![1.0; 2] };
        assert_eq!(short.rgba8().len(), 3 * 3 * 4);
    }

    #[test]
    fn noise_lattice_wraps() {
        for i in 0..8 {
            let y = i as f64 / 8.0;
            assert!((fbm(0.0, y, 4, 4, 3, 7) - fbm(1.0, y, 4, 4, 3, 7)).abs() < 1e-12);
            assert!((fbm(y, 0.0, 4, 4, 3, 7) - fbm(y, 1.0, 4, 4, 3, 7)).abs() < 1e-12);
        }
    }

    #[test]
    fn load_downscales_an_oversized_image_instead_of_retaining_it() {
        let dir = std::env::temp_dir().join("ringdesign_alpha_cap_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("big.png");
        // Compresses to a few KB but would expand to 4 MB of f32 uncapped.
        let big = MAX_ALPHA_EDGE * 2;
        let buf = image::GrayImage::from_pixel(big as u32, big as u32, image::Luma([128u8]));
        buf.save(&path).unwrap();

        let a = Alpha::load(&path).unwrap();
        assert!(
            a.width <= MAX_ALPHA_EDGE && a.height <= MAX_ALPHA_EDGE,
            "kept {}x{}, cap is {MAX_ALPHA_EDGE}",
            a.width,
            a.height
        );
        assert_eq!(a.data.len(), a.width * a.height);
        assert_eq!(a.name, "big");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_rejects_an_image_past_the_hard_edge_without_decoding_it() {
        let dir = std::env::temp_dir().join("ringdesign_alpha_cap_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("huge.png");
        let edge = (HARD_MAX_ALPHA_EDGE + 1) as u32;
        // One row tall, so the file itself stays small.
        let buf = image::GrayImage::from_pixel(edge, 1, image::Luma([0u8]));
        buf.save(&path).unwrap();

        assert!(Alpha::load(&path).is_err(), "an oversized image was accepted");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn thumbnail_rgba8_downscales_and_stays_consistent() {
        let a = Procedural::Rope.generate(256);
        let (w, h, bytes) = a.thumbnail_rgba8(64);
        assert!(w <= 64 && h <= 64, "{w}x{h}");
        assert_eq!(bytes.len(), w * h * 4);
        assert!(bytes.chunks_exact(4).all(|p| p[3] == 255), "not opaque");
        // Grayscale: the three colour channels agree.
        assert!(bytes.chunks_exact(4).all(|p| p[0] == p[1] && p[1] == p[2]));

        // Already under the cap: returned untouched.
        let small = Procedural::Beads.generate(32);
        let (w, h, _) = small.thumbnail_rgba8(64);
        assert_eq!((w, h), (32, 32));
    }

    #[test]
    fn load_dir_stops_at_the_entry_ceiling() {
        let mut lib = AlphaLibrary::default();
        for i in 0..MAX_LIBRARY_ENTRIES {
            lib.insert(Alpha::new(format!("f{i}"), 1, 1, vec![0.0]));
        }
        assert_eq!(lib.len(), MAX_LIBRARY_ENTRIES);

        let dir = std::env::temp_dir().join("ringdesign_alpha_full_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("one_more.png");
        image::GrayImage::from_pixel(8, 8, image::Luma([200u8])).save(&path).unwrap();

        assert_eq!(lib.load_dir(&dir).unwrap(), 0, "ceiling was not enforced");
        assert_eq!(lib.len(), MAX_LIBRARY_ENTRIES);
        let _ = std::fs::remove_file(&path);
    }

    /// A `2r+1` square of 1.0 centred in an otherwise black `n x n` field.
    fn blob(n: usize, r: usize) -> Alpha {
        let c = n / 2;
        let mut data = vec![0.0f32; n * n];
        for j in c - r..=c + r {
            for i in c - r..=c + r {
                data[j * n + i] = 1.0;
            }
        }
        Alpha::new("blob", n, n, data)
    }

    #[test]
    fn crop_of_the_full_rect_is_close_to_identity() {
        let a = Procedural::Braid.generate(64);
        let c = a.crop(CropRect::default());
        assert_eq!((c.width, c.height), (a.width, a.height));
        let worst =
            c.data.iter().zip(&a.data).map(|(x, y)| (x - y).abs()).fold(0.0f32, f32::max);
        assert!(worst < 1e-5, "crop drifted by {worst}");
    }

    #[test]
    fn crop_keeps_source_density_and_survives_bad_rects() {
        let a = Procedural::Beads.generate(64);
        let half = a.crop(CropRect { x0: 0.25, y0: 0.0, x1: 0.75, y1: 1.0 });
        assert_eq!((half.width, half.height), (33, 64));
        assert_eq!(half.data.len(), 33 * 64);
        // An inverted rect is the same region; a degenerate one is a no-op.
        let inv = a.crop(CropRect { x0: 0.75, y0: 1.0, x1: 0.25, y1: 0.0 });
        assert_eq!(inv.data, half.data);
        assert_eq!(a.crop(CropRect { x0: 0.5, y0: 0.2, x1: 0.5, y1: 0.8 }).data, a.data);
        assert_eq!(a.crop(CropRect { x0: f64::NAN, y0: 0.0, x1: 1.0, y1: 1.0 }).data, a.data);
        // A sliver still yields at least one column.
        assert!(a.crop(CropRect { x0: 0.5, y0: 0.5, x1: 0.5001, y1: 0.5001 }).width >= 1);
    }

    #[test]
    fn auto_trim_returns_roughly_the_blob() {
        let a = blob(64, 5);
        let b = a.content_bounds(0.5).unwrap();
        assert!((b.x0 - 27.0 / 63.0).abs() < 1e-9, "{b:?}");
        assert!((b.x1 - 37.0 / 63.0).abs() < 1e-9, "{b:?}");
        assert!((b.y0 - b.x0).abs() < 1e-9 && (b.y1 - b.x1).abs() < 1e-9);

        let t = a.auto_trim(0.5, 0.0);
        assert!((10..=12).contains(&t.width), "{}x{}", t.width, t.height);
        assert_eq!(t.width, t.height);
        assert!(t.data.iter().all(|&v| v > 0.9), "dead border survived");

        let p = a.auto_trim(0.5, 0.1);
        assert!(p.width > t.width);
        assert!(p.data.iter().any(|&v| v < 0.1), "padding kept no margin");

        let black = Alpha::new("k", 8, 8, vec![0.0; 64]);
        assert!(black.content_bounds(0.0).is_none());
        assert_eq!(black.auto_trim(0.0, 0.05).data, black.data);
        assert!(Alpha::default().content_bounds(0.5).is_none());
    }

    #[test]
    fn mirror_tile_kills_the_seam_on_the_mirrored_axis() {
        for &p in Procedural::ALL {
            let a = p.generate(64);
            let m = a.mirror_tile(Axis::Horizontal);
            assert_eq!((m.width, m.height), (128, 64), "{}", p.label());
            assert_eq!(m.data.len(), 128 * 64);
            assert!(m.seam_error().0 < 1e-9, "{} seams: {:?}", p.label(), m.seam_error());
            // Column x of the right half mirrors the left.
            for j in [0usize, 17, 63] {
                for i in 0..64 {
                    assert_eq!(m.data[j * 128 + 127 - i], m.data[j * 128 + i], "{}", p.label());
                }
            }
            let v = a.mirror_tile(Axis::Vertical);
            assert_eq!((v.width, v.height), (64, 128), "{}", p.label());
            assert!(v.seam_error().1 < 1e-9, "{} seams: {:?}", p.label(), v.seam_error());
            let b = a.mirror_tile(Axis::Both);
            assert_eq!((b.width, b.height), (128, 128), "{}", p.label());
            let (hx, vy) = b.seam_error();
            assert!(hx < 1e-9 && vy < 1e-9, "{} seams: {hx} {vy}", p.label());
        }
    }

    #[test]
    fn mirror_tile_makes_a_gradient_tile() {
        let g = ramp(48, 48);
        assert!(g.seam_error().0 > 0.9, "the fixture is not a gradient");
        let m = g.mirror_tile(Axis::Horizontal);
        assert_eq!((m.width, m.height), (96, 48));
        assert!(m.seam_error().0 < 1e-9, "{:?}", m.seam_error());
        assert_eq!(m.data[0], m.data[95]);
        assert_eq!(m.data[47], m.data[48]);
    }

    #[test]
    fn mirror_tile_and_resized_stay_under_the_cap() {
        let big = ramp(MAX_ALPHA_EDGE, MAX_ALPHA_EDGE);
        let m = big.mirror_tile(Axis::Both);
        let (mw, mh) = (m.width, m.height);
        assert!(mw <= MAX_ALPHA_EDGE && mh <= MAX_ALPHA_EDGE, "{mw}x{mh}");
        assert_eq!(m.data.len(), m.width * m.height);
        let (hx, vy) = m.seam_error();
        assert!(hx < 1e-9 && vy < 1e-9, "{hx} {vy}");

        let r = big.resized(4096, 3);
        assert_eq!((r.width, r.height), (MAX_ALPHA_EDGE, 3));
        assert_eq!(r.data.len(), MAX_ALPHA_EDGE * 3);
        assert_eq!(big.resized(0, 0).width, 1);
        assert_eq!(big.resized(MAX_ALPHA_EDGE, MAX_ALPHA_EDGE).data, big.data);
    }

    #[test]
    fn resized_preserves_a_gradient() {
        let a = ramp(64, 8);
        let s = a.resized(16, 4);
        assert_eq!((s.width, s.height), (16, 4));
        assert!(s.data[0].abs() < 1e-6 && (s.data[15] - 1.0).abs() < 1e-6);
        for j in 0..4 {
            for i in 0..15 {
                assert!(s.data[j * 16 + i] < s.data[j * 16 + i + 1], "not monotonic");
            }
        }
    }

    #[test]
    fn flipped_twice_is_identity() {
        let a = Procedural::Feather.generate(32);
        for ax in [Axis::Horizontal, Axis::Vertical, Axis::Both] {
            let f = a.flipped(ax);
            assert_eq!((f.width, f.height), (a.width, a.height));
            assert_eq!(f.flipped(ax).data, a.data);
        }
        assert_eq!(a.flipped(Axis::Horizontal).data[0], a.data[31]);
        assert_eq!(a.flipped(Axis::Vertical).data[0], a.data[31 * 32]);
    }

    #[test]
    fn rotated_four_times_is_identity() {
        let a = ramp(8, 5);
        assert_eq!(a.rotated(0).data, a.data);
        assert_eq!(a.rotated(4).data, a.data);
        assert_eq!(a.rotated(8).data, a.data);
        let q = a.rotated(1);
        assert_eq!((q.width, q.height), (5, 8));
        assert_eq!(q.rotated(3).data, a.data);
        assert_eq!(a.rotated(9).data, q.data);
        assert_eq!(a.rotated(2).data, a.flipped(Axis::Both).data);
        // A quarter turn takes the top-left sample to the top-right.
        assert_eq!(q.data[q.width - 1], a.data[0]);
    }

    #[test]
    fn edge_fade_zeroes_the_border() {
        let a = Alpha::new("f", 16, 16, vec![1.0; 256]);
        let f = a.edge_fade(0.25, Axis::Both);
        assert_eq!((f.width, f.height), (16, 16));
        for k in 0..16 {
            assert_eq!(f.data[k], 0.0, "top row {k}");
            assert_eq!(f.data[15 * 16 + k], 0.0, "bottom row {k}");
            assert_eq!(f.data[k * 16], 0.0, "left column {k}");
            assert_eq!(f.data[k * 16 + 15], 0.0, "right column {k}");
        }
        assert!((f.data[8 * 16 + 8] - 1.0).abs() < 1e-6, "the middle was faded");
        assert!(f.data.iter().all(|v| (0.0..=1.0).contains(v)));
        // One axis leaves the other alone; no fade is a no-op.
        let h = a.edge_fade(0.25, Axis::Horizontal);
        assert_eq!(h.data[8 * 16], 0.0);
        assert_eq!(h.data[8], 1.0);
        assert_eq!(a.edge_fade(0.0, Axis::Both).data, a.data);
        assert_eq!(a.edge_fade(f64::NAN, Axis::Both).data, a.data);
    }

    #[test]
    fn edge_fade_ramps_monotonically_inward() {
        let a = Alpha::new("f", 32, 1, vec![1.0; 32]);
        let f = a.edge_fade(0.4, Axis::Horizontal);
        for i in 0..15 {
            assert!(f.data[i] <= f.data[i + 1], "not rising at {i}");
        }
        assert!(f.data[3] > 0.0 && f.data[3] < 1.0, "no ramp: {}", f.data[3]);
    }

    #[test]
    fn levels_clamps_and_shapes() {
        let a = Alpha::new("l", 5, 1, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
        let l = a.levels(0.25, 0.75, 1.0);
        assert_eq!(l.data, vec![0.0, 0.0, 0.5, 1.0, 1.0]);
        assert_eq!((l.width, l.height), (a.width, a.height));
        let g = a.levels(0.0, 1.0, 2.0);
        assert!((g.data[2] - 0.25).abs() < 1e-6, "{}", g.data[2]);
        assert!(g.data.iter().all(|v| (0.0..=1.0).contains(v)));
        // Degenerate or non-finite ranges are left alone.
        assert_eq!(a.levels(0.5, 0.5, 1.0).data, a.data);
        assert_eq!(a.levels(0.9, 0.1, 1.0).data, a.data);
        assert_eq!(a.levels(f32::NAN, 1.0, 1.0).data, a.data);
        assert!(a.levels(0.0, 1.0, f32::NAN).data.iter().all(|v| v.is_finite()));
        let nanny = Alpha::new("n", 2, 1, vec![f32::NAN, 0.5]);
        assert!(nanny.levels(0.0, 1.0, 1.0).data.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn seam_error_reads_low_for_a_tiling_alpha_and_high_for_a_gradient() {
        let (hx, vy) = Procedural::GreekKey.generate(128).seam_error();
        assert!(hx < 1e-6 && vy < 1e-6, "{hx} {vy}");
        let (hx, vy) = ramp(64, 64).seam_error();
        assert!(hx > 0.9, "horizontal gradient not flagged: {hx}");
        assert!(vy < 1e-6, "vertical axis of a horizontal gradient: {vy}");
        assert_eq!(Alpha::default().seam_error(), (0.0, 0.0));
        assert_eq!(Alpha::new("p", 1, 1, vec![0.7]).seam_error(), (0.0, 0.0));
    }

    #[test]
    fn editing_ops_survive_malformed_alphas() {
        let liars = vec![
            Alpha { name: "short".into(), width: 4, height: 4, data: vec![0.5; 3] },
            Alpha { name: "long".into(), width: 2, height: 2, data: vec![0.5; 99] },
            Alpha { name: "zero_w".into(), width: 0, height: 4, data: vec![0.5; 4] },
            Alpha { name: "nan".into(), width: 2, height: 2, data: vec![f32::NAN, 0.0, 1.0, -3.0] },
            Alpha { name: "one".into(), width: 1, height: 1, data: vec![0.25] },
            Alpha { name: "row".into(), width: 5, height: 1, data: vec![0.1; 5] },
            Alpha { name: "col".into(), width: 1, height: 5, data: vec![0.1; 5] },
        ];
        // Every op either builds a consistent alpha or hands the input back.
        let check = |out: &Alpha, src: &Alpha| {
            let untouched = (out.width, out.height, out.data.len())
                == (src.width, src.height, src.data.len());
            assert!(
                untouched || out.data.len() == out.width * out.height,
                "{}: {}x{} with {} samples",
                src.name,
                out.width,
                out.height,
                out.data.len()
            );
            assert!(out.width <= MAX_ALPHA_EDGE && out.height <= MAX_ALPHA_EDGE, "{}", src.name);
        };
        for a in &liars {
            for ax in [Axis::Horizontal, Axis::Vertical, Axis::Both] {
                for out in [a.flipped(ax), a.mirror_tile(ax), a.edge_fade(0.3, ax)] {
                    check(&out, a);
                }
            }
            for t in [-1.0, 0.0, 0.5, 1.0, f32::NAN] {
                let _ = a.content_bounds(t);
                check(&a.auto_trim(t, 0.05), a);
            }
            for q in [0u32, 1, 2, 3, 7, u32::MAX] {
                let out = a.rotated(q);
                assert!(out.data.len() == out.width * out.height || out.data.len() == a.data.len());
            }
            let _ = a.seam_error();
            check(&a.levels(0.1, 0.9, 1.5), a);
            check(&a.resized(3, 7), a);
            check(&a.crop(CropRect { x0: 0.1, y0: 0.1, x1: 0.9, y1: 0.9 }), a);
        }
    }

    #[test]
    fn mirror_tile_can_never_outgrow_the_cap() {
        // Sizes that straddle the cap and its half, on both axes.
        let edges = [1usize, 2, 255, 256, 257, 383, 511, MAX_ALPHA_EDGE];
        let mut worst = 0usize;
        for &w in &edges {
            for &h in &edges {
                let src = Alpha::new("t", w, h, vec![0.5; w * h]);
                for ax in [Axis::Horizontal, Axis::Vertical, Axis::Both] {
                    let m = src.mirror_tile(ax);
                    assert_eq!(m.data.len(), m.width * m.height, "{w}x{h} {ax:?}");
                    assert!(
                        m.width <= MAX_ALPHA_EDGE && m.height <= MAX_ALPHA_EDGE,
                        "{w}x{h} {ax:?} grew to {}x{}",
                        m.width,
                        m.height
                    );
                    worst = worst.max(m.data.len());
                    // Mirroring twice must not compound past the cap either.
                    let t = m.mirror_tile(Axis::Both).mirror_tile(Axis::Both);
                    assert!(t.width <= MAX_ALPHA_EDGE && t.height <= MAX_ALPHA_EDGE);
                }
            }
        }
        assert_eq!(worst, MAX_ALPHA_EDGE * MAX_ALPHA_EDGE);
        println!("worst mirror_tile allocation: {worst} f32 = {} KiB", worst * 4 / 1024);
    }

    #[test]
    fn scratch_mirror_tile_proof() {
        let mut worst_after = 0.0f64;
        println!("== builtin procedurals: seam_error before -> after mirror_tile(Both) ==");
        for &p in Procedural::ALL {
            let a = p.generate(256);
            let (b0, b1) = a.seam_error();
            let m = a.mirror_tile(Axis::Both);
            let (a0, a1) = m.seam_error();
            worst_after = worst_after.max(a0).max(a1);
            println!(
                "{:14} {}x{} before ({b0:.6}, {b1:.6}) -> {}x{} after ({a0:.3e}, {a1:.3e})",
                p.label(),
                a.width,
                a.height,
                m.width,
                m.height
            );
        }

        // `library::alpha_dir()` at run time, not `env!("HOME")` at compile
        // time — that baked one developer's home directory into the binary.
        let dirs = crate::library::alpha_dirs();
        let picks = [
            "ornament-a-01.png",
            "ornament-a-02.png",
            "ornament-a-03.png",
            "scale-01.png",
            "crack-01.png",
            "crack-07.png",
        ];
        println!("== imported alphas from {dirs:?} ==");
        let mut real = 0;
        for name in picks {
            let Some(a) = dirs.iter().find_map(|d| Alpha::load(d.join(name)).ok()) else {
                println!("{name:28} MISSING");
                continue;
            };
            real += 1;
            let (b0, b1) = a.seam_error();
            let m = a.mirror_tile(Axis::Both);
            let (a0, a1) = m.seam_error();
            worst_after = worst_after.max(a0).max(a1);
            // A harvested fragment: crop, trim, then mirror.
            let frag = a
                .crop(CropRect { x0: 0.18, y0: 0.22, x1: 0.62, y1: 0.71 })
                .auto_trim(0.06, 0.02);
            let (f0, f1) = frag.seam_error();
            let ft = frag.mirror_tile(Axis::Both);
            let (t0, t1) = ft.seam_error();
            worst_after = worst_after.max(t0).max(t1);
            println!(
                "{name:28} {}x{} before ({b0:.4}, {b1:.4}) -> {}x{} after ({a0:.3e}, {a1:.3e}) | fragment {}x{} before ({f0:.4}, {f1:.4}) -> {}x{} after ({t0:.3e}, {t1:.3e})",
                a.width, a.height, m.width, m.height,
                frag.width, frag.height, ft.width, ft.height
            );
        }
        println!("real alphas measured: {real}");
        println!("WORST seam_error after mirror_tile(Both): {worst_after:.3e}");
        // The imported half of this probe reads the *user's* alpha directory,
        // which exists on the machine that harvested those files and nowhere
        // else. Asserting on it made the suite pass or fail on what happened
        // to be in one person's home directory — which is exactly what CI
        // found the first time it ran. Measure them when they are there, say
        // so when they are not, and gate only on what is reproducible.
        if real == 0 {
            println!("no imported alphas on this machine — builtins only");
        }
        assert!(worst_after < 1e-9, "mirror_tile left a seam: {worst_after}");
    }

    #[test]
    fn a_harvested_fragment_becomes_a_castable_tile() {
        // Crop a fragment off a motif, trim its dead border, mirror it into a
        // tile, then fade the across-band edges so nothing ends on a wall.
        let src = Procedural::Floral.generate(128);
        let frag = src.crop(CropRect { x0: 0.12, y0: 0.30, x1: 0.61, y1: 0.74 });
        let tile = frag
            .auto_trim(0.05, 0.02)
            .mirror_tile(Axis::Horizontal)
            .edge_fade(0.2, Axis::Vertical);
        assert!(tile.width <= MAX_ALPHA_EDGE && tile.height <= MAX_ALPHA_EDGE);
        assert_eq!(tile.data.len(), tile.width * tile.height);
        assert!(tile.seam_error().0 < 1e-9, "the tile still seams: {:?}", tile.seam_error());
        for i in 0..tile.width {
            assert_eq!(tile.data[i], 0.0);
            assert_eq!(tile.data[(tile.height - 1) * tile.width + i], 0.0);
        }
        assert!(tile.data.iter().any(|&v| v > 0.5), "the fragment was empty");
    }
}

/// A normalized sub-rectangle of an alpha, 0..1 in both axes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CropRect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Default for CropRect {
    fn default() -> Self {
        Self { x0: 0.0, y0: 0.0, x1: 1.0, y1: 1.0 }
    }
}

impl CropRect {
    pub fn width(&self) -> f64 {
        (self.x1 - self.x0).abs()
    }
    pub fn height(&self) -> f64 {
        (self.y1 - self.y0).abs()
    }
    /// Ordered corners clamped into 0..1.
    pub fn normalized(&self) -> CropRect {
        CropRect {
            x0: self.x0.min(self.x1).clamp(0.0, 1.0),
            y0: self.y0.min(self.y1).clamp(0.0, 1.0),
            x1: self.x0.max(self.x1).clamp(0.0, 1.0),
            y1: self.y0.max(self.y1).clamp(0.0, 1.0),
        }
    }
}

/// Which axis an operation applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    Horizontal,
    Vertical,
    Both,
}

/// Inclusive 0..1 position of index `i` among `n` samples; the centre if `n` is 1.
#[inline]
fn axis_t(i: usize, n: usize) -> f64 {
    if n > 1 { i as f64 / (n - 1) as f64 } else { 0.5 }
}

/// Index range `lo..=hi` of an `n`-sample axis as an inclusive 0..1 span.
#[inline]
fn span01(lo: usize, hi: usize, n: usize) -> (f64, f64) {
    if n > 1 {
        let d = (n - 1) as f64;
        (lo as f64 / d, hi as f64 / d)
    } else {
        (0.0, 1.0)
    }
}

/// Output samples for an inclusive 0..1 `span` of an `n`-sample axis, at source
/// density, capped by [`MAX_ALPHA_EDGE`].
#[inline]
fn span_samples(span: f64, n: usize) -> usize {
    let taken = (span * n.saturating_sub(1) as f64).round().max(0.0) as usize;
    taken.saturating_add(1).clamp(1, MAX_ALPHA_EDGE)
}

impl Alpha {
    /// True when the declared extent matches the buffer and is non-empty.
    fn is_sane(&self) -> bool {
        !self.is_empty() && self.data.len() == self.width * self.height
    }

    /// Take a sub-rectangle as a new alpha.
    pub fn crop(&self, rect: CropRect) -> Alpha {
        if !self.is_sane() {
            return self.clone();
        }
        let r = rect.normalized();
        let finite = r.x0.is_finite() && r.y0.is_finite() && r.x1.is_finite() && r.y1.is_finite();
        if !finite || r.width() < 1e-9 || r.height() < 1e-9 {
            return self.clone();
        }
        let (ow, oh) = (span_samples(r.width(), self.width), span_samples(r.height(), self.height));
        let mut data = Vec::with_capacity(ow * oh);
        for j in 0..oh {
            let y = r.y0 + r.height() * axis_t(j, oh);
            for i in 0..ow {
                data.push(self.sample(r.x0 + r.width() * axis_t(i, ow), y));
            }
        }
        Alpha::new(self.name.clone(), ow, oh, data)
    }

    /// Bounding box of everything above `threshold`, for trimming dead border.
    pub fn content_bounds(&self, threshold: f32) -> Option<CropRect> {
        if !self.is_sane() {
            return None;
        }
        let (w, h) = (self.width, self.height);
        let (mut i0, mut j0, mut i1, mut j1) = (w, h, 0usize, 0usize);
        for j in 0..h {
            let row = j * w;
            for i in 0..w {
                let v = self.data[row + i];
                if v.is_finite() && v > threshold {
                    i0 = i0.min(i);
                    i1 = i1.max(i);
                    j0 = j0.min(j);
                    j1 = j1.max(j);
                }
            }
        }
        if i0 > i1 || j0 > j1 {
            return None;
        }
        let (x0, x1) = span01(i0, i1, w);
        let (y0, y1) = span01(j0, j1, h);
        Some(CropRect { x0, y0, x1, y1 })
    }

    /// Crop away a uniform border below `threshold`, keeping `pad` of the image
    /// as margin on each side.
    pub fn auto_trim(&self, threshold: f32, pad: f64) -> Alpha {
        let Some(b) = self.content_bounds(threshold) else {
            return self.clone();
        };
        let p = if pad.is_finite() { pad.clamp(0.0, 0.5) } else { 0.0 };
        self.crop(CropRect { x0: b.x0 - p, y0: b.y0 - p, x1: b.x1 + p, y1: b.y1 + p })
    }

    pub fn flipped(&self, axis: Axis) -> Alpha {
        if !self.is_sane() {
            return self.clone();
        }
        let (w, h) = (self.width, self.height);
        let fx = matches!(axis, Axis::Horizontal | Axis::Both);
        let fy = matches!(axis, Axis::Vertical | Axis::Both);
        let mut data = Vec::with_capacity(w * h);
        for j in 0..h {
            let row = if fy { h - 1 - j } else { j } * w;
            for i in 0..w {
                data.push(self.data[row + if fx { w - 1 - i } else { i }]);
            }
        }
        Alpha::new(self.name.clone(), w, h, data)
    }

    /// Rotate by whole quarter turns.
    pub fn rotated(&self, quarter_turns: u32) -> Alpha {
        let mut out = self.clone();
        if !self.is_sane() {
            return out;
        }
        for _ in 0..quarter_turns % 4 {
            let (w, h) = (out.width, out.height);
            let mut data = Vec::with_capacity(w * h);
            for j in 0..w {
                for i in 0..h {
                    data.push(out.data[(h - 1 - i) * w + j]);
                }
            }
            out = Alpha::new(out.name.clone(), h, w, data);
        }
        out
    }

    /// Mirror the image against itself so opposite edges match exactly.
    ///
    /// This is what turns a harvested fragment into a usable tile: `[A|flip(A)]`
    /// repeats with no seam, because the tile's two outer edges are the same
    /// edge of the source. Doubles the extent on each mirrored axis.
    pub fn mirror_tile(&self, axis: Axis) -> Alpha {
        if !self.is_sane() {
            return self.clone();
        }
        let mx = matches!(axis, Axis::Horizontal | Axis::Both);
        let my = matches!(axis, Axis::Vertical | Axis::Both);
        // Halve the source on a doubled axis so the composite still fits the cap.
        let half = (MAX_ALPHA_EDGE / 2).max(1);
        let tw = self.width.min(if mx { half } else { MAX_ALPHA_EDGE });
        let th = self.height.min(if my { half } else { MAX_ALPHA_EDGE });
        let src = self.resized(tw, th);
        let (sw, sh) = (src.width, src.height);
        let (ow, oh) = (if mx { sw * 2 } else { sw }, if my { sh * 2 } else { sh });
        let mut data = Vec::with_capacity(ow * oh);
        for j in 0..oh {
            let row = if j < sh { j } else { 2 * sh - 1 - j } * sw;
            for i in 0..ow {
                data.push(src.data[row + if i < sw { i } else { 2 * sw - 1 - i }]);
            }
        }
        Alpha::new(self.name.clone(), ow, oh, data)
    }

    /// Fade the outer `frac` of each edge to zero, so a fragment sits down into
    /// the surface instead of ending on a wall.
    pub fn edge_fade(&self, frac: f64, axis: Axis) -> Alpha {
        if !self.is_sane() || !frac.is_finite() || frac <= 0.0 {
            return self.clone();
        }
        let f = frac.min(0.5);
        let (w, h) = (self.width, self.height);
        let ramp = |i: usize, n: usize| {
            let d = if n > 1 { i.min(n - 1 - i) as f64 / (n - 1) as f64 } else { 0.0 };
            smoothstep(0.0, f, d) as f32
        };
        let fade = |on: bool, n: usize| -> Vec<f32> {
            (0..n).map(|i| if on { ramp(i, n) } else { 1.0 }).collect()
        };
        let cols = fade(matches!(axis, Axis::Horizontal | Axis::Both), w);
        let rows = fade(matches!(axis, Axis::Vertical | Axis::Both), h);
        let mut data = Vec::with_capacity(w * h);
        for (j, &ry) in rows.iter().enumerate() {
            let row = j * w;
            for (i, &rx) in cols.iter().enumerate() {
                let v = self.data[row + i];
                data.push(if v.is_finite() { v * ry * rx } else { 0.0 });
            }
        }
        Alpha::new(self.name.clone(), w, h, data)
    }

    /// Remap levels: `lo`/`hi` rescale the input range, `gamma` shapes it.
    pub fn levels(&self, lo: f32, hi: f32, gamma: f32) -> Alpha {
        if !self.is_sane() || !lo.is_finite() || !hi.is_finite() || hi - lo < 1e-6 {
            return self.clone();
        }
        let g = if gamma.is_finite() { gamma.clamp(0.05, 8.0) } else { 1.0 };
        let inv = 1.0 / (hi - lo);
        let data = self
            .data
            .iter()
            .map(|&v| {
                let t = if v.is_finite() { ((v - lo) * inv).clamp(0.0, 1.0) } else { 0.0 };
                if (g - 1.0).abs() > 1e-9 { t.powf(g).clamp(0.0, 1.0) } else { t }
            })
            .collect();
        Alpha::new(self.name.clone(), self.width, self.height, data)
    }

    pub fn resized(&self, width: usize, height: usize) -> Alpha {
        if !self.is_sane() {
            return self.clone();
        }
        let ow = width.clamp(1, MAX_ALPHA_EDGE);
        let oh = height.clamp(1, MAX_ALPHA_EDGE);
        if (ow, oh) == (self.width, self.height) {
            return self.clone();
        }
        let mut data = Vec::with_capacity(ow * oh);
        for j in 0..oh {
            let y = axis_t(j, oh);
            for i in 0..ow {
                data.push(self.sample(axis_t(i, ow), y));
            }
        }
        Alpha::new(self.name.clone(), ow, oh, data)
    }

    pub fn renamed(&self, name: impl Into<String>) -> Alpha {
        Alpha { name: name.into(), ..self.clone() }
    }

    /// Mean absolute mismatch between opposing edges, per axis, 0..1.
    ///
    /// The number the editor shows to say whether something will actually tile:
    /// near 0 is seamless, above roughly 0.05 shows a visible joint.
    pub fn seam_error(&self) -> (f64, f64) {
        if !self.is_sane() {
            return (0.0, 0.0);
        }
        let (w, h) = (self.width, self.height);
        let at = |k: usize| -> f64 {
            let v = self.data[k];
            if v.is_finite() { v.clamp(0.0, 1.0) as f64 } else { 0.0 }
        };
        let hx = if w > 1 {
            (0..h).map(|j| (at(j * w) - at(j * w + w - 1)).abs()).sum::<f64>() / h as f64
        } else {
            0.0
        };
        let vy = if h > 1 {
            (0..w).map(|i| (at(i) - at((h - 1) * w + i)).abs()).sum::<f64>() / w as f64
        } else {
            0.0
        };
        (hx.clamp(0.0, 1.0), vy.clamp(0.0, 1.0))
    }

}

#[cfg(test)]
mod feature_tests {
    use super::*;

    #[test]
    fn stripes_measure_their_own_width_and_gap() {
        let n = 64;
        let data: Vec<f32> = (0..n * n).map(|i| if (i % n) % 16 < 4 { 1.0 } else { 0.0 }).collect();
        let a = Alpha::new("stripes", n, n, data);
        let (ink, gap) = a.min_feature_px().unwrap();
        assert!((ink - 4.0).abs() <= 1.0, "ink {ink}");
        assert!((gap - 12.0).abs() <= 2.0, "gap {gap}");
        let blank = Alpha::new("blank", 8, 8, vec![0.0; 64]);
        assert!(blank.min_feature_px().is_none());
        let disc: Vec<f32> = (0..n * n).map(|i| { let (x, y) = ((i % n) as f64 - 32.0, (i / n) as f64 - 32.0); if x * x + y * y < 100.0 { 1.0 } else { 0.0 } }).collect();
        let (ink, _) = Alpha::new("disc", n, n, disc).min_feature_px().unwrap();
        assert!((ink - 20.0).abs() <= 2.0, "a 20 px disc: {ink}");
    }
}
