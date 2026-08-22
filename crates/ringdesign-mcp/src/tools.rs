//! The MCP tool surface: inspect, mutate, analyse, and export the shared
//! design.
//!
//! Every tool takes the engine lock once, does its work, drops it, then calls
//! [`RingDesignServer::touch`]. The lock is a non-reentrant
//! `parking_lot::Mutex`, so no tool may call another while holding it.

use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{CastReport, Section};
use ringdesign_core::field::{
    Blend, BorderLayer, BorderProfile, Layer, LayerEntry, MilgrainLayer,
    SIDE_FACE_MIN_DRAFT_DEG, SeatPadLayer, SignetLayer, SignetOutline,
};
use ringdesign_core::mesh::{BuildParams, Report};
use ringdesign_core::profile::{ProfileStyle, ShankKind};
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{RingDesign, RingSize};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ErrorData, Json, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::RingDesignServer;

/// Most section points returned by `cross_section`.
const SECTION_POINT_CAP: usize = 120;

/// Default rows returned by `list_alphas`.
const ALPHA_LIMIT_DEFAULT: u32 = 100;

/// Largest `limit` `list_alphas` will honour.
const ALPHA_LIMIT_MAX: u32 = 500;

// --- Returned shapes -------------------------------------------------------

#[derive(Debug, Serialize, JsonSchema)]
pub struct DesignJson {
    pub generation: u64,
    /// The whole `RingDesign` as stored, in the same shape a saved file uses.
    pub design: serde_json::Value,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MetalWeightJson {
    pub metal: String,
    pub grams: f64,
    pub dwt: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LayerInfo {
    pub index: usize,
    pub name: String,
    /// Tiling, Border, Gem Seat Pad, or Milgrain.
    pub kind: String,
    pub enabled: bool,
    /// Add, Max, Min, Subtract (carve), or Replace.
    pub blend: String,
    pub opacity: f64,
    /// Present only when the layer is gated to an arc of the ring.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LayerList {
    pub count: usize,
    pub layers: Vec<LayerInfo>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SideFaceInfo {
    /// `[start, end]` of the face at the low `v` edge, mm. Null when that edge
    /// has none, which is the case for a one-sided edge flange.
    pub low_mm: Option<[f64; 2]>,
    /// `[start, end]` of the face at the high `v` edge, mm.
    pub high_mm: Option<[f64; 2]>,
    /// Whether both faces can carry the same band, so a tiling can be mirrored
    /// onto them with mirror_v rather than sitting on one edge.
    pub even: bool,
    pub min_draft_deg: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RingSummary {
    pub generation: u64,
    pub name: String,
    pub size_us: f64,
    pub size_label: String,
    pub inner_diameter_mm: f64,
    pub profile_style: String,
    pub profile_casting_note: String,
    pub width_mm: f64,
    pub thickness_mm: f64,
    /// Radial drop from the crest to the outer edge after clamping, mm.
    pub crown_mm: f64,
    /// Radial metal left at the outer edges, mm.
    pub edge_thickness_mm: f64,
    pub comfort_fit_mm: f64,
    pub side_draft_deg: f64,
    pub shank: String,
    pub shank_amount: f64,
    /// `u` span: arc distance around the ring at the crest radius, mm.
    pub circumference_mm: f64,
    /// `v` span: arc distance across the cross-section, mm.
    pub band_v_len_mm: f64,
    /// `v` of the crest line, mm. Relief here undercuts first.
    pub crest_v_mm: f64,
    /// The `v` runs square enough to the mould pull to hold deep relief, and
    /// the only place ornament survives above roughly 0.15 mm. Null on a dome.
    pub side_faces: Option<SideFaceInfo>,
    pub layers: Vec<LayerInfo>,
    pub outer_diameter_mm: f64,
    pub band_width_mm: f64,
    pub volume_mm3: f64,
    pub max_relief_mm: f64,
    pub min_relief_mm: f64,
    pub silver_925_g: f64,
    pub gold_14k_g: f64,
    pub triangle_count: usize,
    pub watertight: bool,
    pub summary: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ProfileStyleInfo {
    pub style: String,
    pub label: String,
    pub casting_note: String,
    /// Superellipse edge exponent the preset sets.
    pub shape_a: f64,
    /// Superellipse crest exponent the preset sets.
    pub shape_b: f64,
    /// Crown as a fraction of thickness.
    pub crown_fraction: f64,
    /// Edge fillet as a fraction of thickness.
    pub edge_round_fraction: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ProfileStyleList {
    pub styles: Vec<ProfileStyleInfo>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ShankStyleInfo {
    pub kind: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ShankStyleList {
    pub kinds: Vec<ShankStyleInfo>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AlphaInfo {
    pub name: String,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AlphaList {
    /// Entries in the library.
    pub total: usize,
    /// Entries matching `filter`.
    pub matched: usize,
    pub returned: usize,
    pub alphas: Vec<AlphaInfo>,
}

/// Result of a design-level mutation.
#[derive(Debug, Serialize, JsonSchema)]
pub struct DesignChange {
    pub generation: u64,
    /// Fields this call actually set, as `name=value`.
    pub applied: Vec<String>,
    pub summary: String,
}

/// Result of a layer-stack mutation.
#[derive(Debug, Serialize, JsonSchema)]
pub struct LayerChange {
    pub generation: u64,
    /// Index the call landed on, absent when the layer was removed.
    pub index: Option<usize>,
    pub applied: Vec<String>,
    pub layers: Vec<LayerInfo>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReportJson {
    pub generation: u64,
    pub watertight: bool,
    pub triangle_count: usize,
    pub vertex_count: usize,
    pub boundary_edges: usize,
    pub non_manifold_edges: usize,
    pub theta_steps: usize,
    pub profile_steps: usize,
    pub volume_mm3: f64,
    pub surface_area_mm2: f64,
    /// Overall size (x, y, z), mm.
    pub bounds_mm: [f64; 3],
    pub inner_diameter_mm: f64,
    pub outer_diameter_mm: f64,
    pub band_width_mm: f64,
    /// Tallest displacement the layer stack applied, mm.
    pub max_relief_mm: f64,
    /// Deepest carved displacement, mm.
    pub min_relief_mm: f64,
    pub metals: Vec<MetalWeightJson>,
    pub build_ms: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CastJson {
    pub generation: u64,
    /// Castable, Marginal, or NotCastable.
    pub verdict: String,
    pub verdict_label: String,
    pub good_faces: usize,
    pub marginal_faces: usize,
    pub vertical_faces: usize,
    pub undercut_faces: usize,
    pub undercut_area_mm2: f64,
    pub marginal_area_mm2: f64,
    pub total_area_mm2: f64,
    pub undercut_fraction: f64,
    /// Most negative draft found, degrees. Negative leans back under itself.
    pub worst_draft_deg: f64,
    pub parting_z_mm: f64,
    pub min_draft_deg: f64,
    /// Plain-language findings, verbatim.
    pub notes: Vec<String>,
    /// Per-seat bench checks and carat totals, when the design carries seats.
    pub stones: Option<StonesJson>,
    /// The authoritative verdict, sampled off the surface itself rather than
    /// any mesh — build kind and resolution cannot put a phantom in it.
    pub field: FieldJson,
    /// Per-layer design-for-manufacture findings: features finer than the
    /// sand's min_detail_mm cast as mush. "LayerName: message" per finding.
    pub dfm: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FieldJson {
    /// Castable, Marginal, or NotCastable — trust this one over the mesh's.
    pub verdict: String,
    pub verdict_label: String,
    pub worst_draft_deg: f64,
    pub undercut_fraction: f64,
    /// Thinnest outer-to-bore metal over the finger hole, mm.
    pub thinnest_wall_mm: f64,
    pub thinnest_wall_theta_deg: f64,
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StonesJson {
    pub stone_count: u32,
    pub total_carats: f64,
    pub seats: Vec<SeatJson>,
    /// Pairs of stones — from any layers, not just neighbours in one run —
    /// with less than 0.3 mm of metal between them, worst first.
    pub crowding: Vec<StonePairJson>,
    /// How many such pairs there are, including any past the listed few.
    pub tight_pairs: usize,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StonePairJson {
    /// The two seats' labels and where they sit, degrees around the ring.
    pub a: String,
    pub b: String,
    pub a_theta_deg: f64,
    pub b_theta_deg: f64,
    /// Metal between the two girdles, mm. Negative means they overlap.
    pub gap_mm: f64,
    /// The same gap at the shallower culet, where the ring's own curvature
    /// has closed the arc in — the number that decides whether the bridge
    /// fills.
    pub gap_deep_mm: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SeatJson {
    /// Layer name, prefixed by its group path.
    pub label: String,
    /// Boss, Bezel collar, or Gypsy mound.
    pub style: String,
    /// Stations this seat occupies after its window.
    pub count: u32,
    pub seat_diameter_mm: f64,
    /// e.g. "2.5 mm Round brilliant (0.06 ct)"; absent when no stone is assigned.
    pub stone: Option<String>,
    /// "side face" (castable by construction) or "crown +12.3 deg".
    pub sits_on: String,
    pub edge_clearance_mm: f64,
    /// Metal available for the pavilion along the seat's normal, mm.
    pub depth_available_mm: f64,
    /// Runs only: metal left between neighbouring stones, mm.
    pub bridge_mm: Option<f64>,
    /// Shared-prong runs only: e.g. "15 pairs, 0.70 mm posts, 0.90 mm proud".
    pub shared_prongs: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SectionPointJson {
    /// Radius from the ring axis, mm.
    pub r: f64,
    /// Height along the finger axis, mm.
    pub z: f64,
    pub draft_deg: f64,
    /// Good draft, Marginal, Vertical wall, or Undercut.
    pub class: String,
    /// Whether this point lies on the displaceable outer surface.
    pub surface: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SectionJson {
    pub theta_deg: f64,
    pub parting_z_mm: f64,
    pub min_r_mm: f64,
    pub max_r_mm: f64,
    pub min_z_mm: f64,
    pub max_z_mm: f64,
    /// Thinnest metal between the outer surface and the bore, mm.
    pub min_wall_mm: f64,
    pub undercut_count: usize,
    /// Points the slice was sampled at.
    pub sampled_points: usize,
    /// Points returned after downsampling.
    pub returned_points: usize,
    pub points: Vec<SectionPointJson>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ExportResult {
    pub path: String,
    pub bytes: usize,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FileResult {
    pub path: String,
    pub generation: u64,
    pub summary: String,
}

// --- Parameters ------------------------------------------------------------

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct TemplateParams {
    /// Template name, case-insensitive. Omit to list what exists.
    pub name: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SetRingParams {
    /// Design name, carried into exported OBJ files.
    pub name: Option<String>,
    /// US finger size, rounded to the nearest quarter. 0.5 to 20.
    pub size: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SetProfileParams {
    /// Flat, LowDome, HalfRound, HighDome, CushionDome, DShape, Beveled,
    /// KnifeEdge, or Custom. Applied before the other fields in this call.
    pub style: Option<String>,
    /// Axial extent of the band at the bore, mm.
    pub width_mm: Option<f64>,
    /// Maximum radial thickness, at the crest, mm.
    pub thickness_mm: Option<f64>,
    /// Radial drop from the crest to the outer edge, mm. Clamped so the edge
    /// keeps 0.2 mm of metal.
    pub crown_mm: Option<f64>,
    /// Superellipse edge exponent: higher flattens the crown and sharpens the
    /// falloff at the edges.
    pub shape_a: Option<f64>,
    /// Superellipse crest exponent: higher fills the crest out.
    pub shape_b: Option<f64>,
    /// Crest position across the width, -1 at one edge to 1 at the other.
    pub crest_bias: Option<f64>,
    /// Fillet where the side faces meet the outer surface, mm.
    pub edge_round_mm: Option<f64>,
    /// Inward dome of the bore, mm. The stated size is measured at its crown.
    pub comfort_fit_mm: Option<f64>,
    /// Taper of the side faces, degrees. Positive narrows the band outward.
    pub side_draft_deg: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SetShankParams {
    /// Uniform, Tapered, ReverseTaper, Cathedral, EuroFlat, or Signet.
    pub kind: Option<String>,
    /// Strength of the modulation, 0 to 1. On Signet this is how far the shank
    /// narrows: 1 takes it to 16% of the head width.
    pub amount: Option<f64>,
    /// Signet only: plan silhouette of the face — Oval, Round, Cushion,
    /// Rectangle, Hexagon, Octagon, Marquise, Shield, or Heart. The band's own
    /// width follows it.
    pub head_outline: Option<String>,
    /// Signet only: extent of the face around the ring, mm. Its extent across
    /// the band is the profile's width.
    pub head_length_mm: Option<f64>,
    /// Signet only: how far the middle of the table stands above the band's
    /// crest, mm.
    pub head_rise_mm: Option<f64>,
    /// Signet only: arc the crest takes to fall from the head to the shank,
    /// degrees.
    pub head_shoulder_deg: Option<f64>,
    /// Signet only: arc the band's *width* takes to come back to the shank,
    /// degrees. Much longer than the face — this is the swell a signet reads as
    /// from the side.
    pub head_swell_deg: Option<f64>,
    /// Signet only: how far the body under the table rounds away from the
    /// face's outline, 0 to 1. 0 extrudes the face down to the finger; 1 leaves
    /// the shape on the table and fairs everything beneath it.
    pub head_body_fair: Option<f64>,
    /// Signet only: 1 makes the table a true plane to engrave; below that the
    /// head keeps the profile's own crown and stays domed.
    pub head_table_flat: Option<f64>,
    /// Signet only: rounding between the table and the head's walls, mm — how
    /// hard the face outline reads. The reference signets round theirs about
    /// 0.6 mm; the outline is the one edge a signet has.
    pub head_rim_round_mm: Option<f64>,
    /// Signet only: where the head sits round the ring, degrees. 90 is the top.
    pub head_theta_deg: Option<f64>,
    /// Signet only: outline of a second head — the toi et moi. Set to add or
    /// change it; the string "none" removes it.
    pub second_head_outline: Option<String>,
    /// Signet only: the second head's face length around the ring, mm.
    pub second_head_length_mm: Option<f64>,
    /// Signet only: where the second head sits, degrees.
    pub second_head_theta_deg: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SetCastingParams {
    /// Height of the parting plane, mm. Ignored while auto_parting is on.
    pub parting_z_mm: Option<f64>,
    /// Draft below which a wall is called marginal, degrees.
    pub min_draft_deg: Option<f64>,
    /// Put the parting plane at the widest silhouette of the ring.
    pub auto_parting: Option<bool>,
    /// Thinnest section expected to fill, mm.
    pub min_section_mm: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SetBuildParamsParams {
    /// Sweep steps around the ring. Clamped to 24..4096 when building.
    pub theta_steps: Option<u32>,
    /// Vertices around the cross-section. Clamped to 24..1024 when building.
    pub profile_steps: Option<u32>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct BuildParamsOverride {
    /// One-off sweep steps for this build; the design keeps its own value.
    pub theta_steps: Option<u32>,
    /// One-off cross-section vertices for this build.
    pub profile_steps: Option<u32>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ListAlphasParams {
    /// Keep only names containing this substring, case-insensitive.
    pub filter: Option<String>,
    /// Rows to return. Default 100, maximum 500.
    pub limit: Option<u32>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct AddTilingParams {
    /// Name of an alpha from list_alphas.
    pub alpha: String,
    /// Layer name in the stack. Defaults to the alpha's name.
    pub name: Option<String>,
    /// Add, Max, Min, Subtract, or Replace. Defaults to Max.
    pub blend: Option<String>,
    /// Scale on this layer's output. Defaults to 1.
    pub opacity: Option<f64>,
    /// Tiles around the circumference. Integer, so the pattern is seamless.
    pub repeats_around: Option<u32>,
    /// Tile rows stacked across the band.
    pub rows: Option<u32>,
    /// Centre of the tiled strip across the cross-section, mm of `v`.
    pub v_center_mm: Option<f64>,
    /// Total `v` extent the tiling covers, mm.
    pub v_span_mm: Option<f64>,
    /// Rotation of the alpha inside its cell, degrees.
    pub rotation_deg: Option<f64>,
    /// Lattice shift along the ring, in fractions of a cell.
    pub offset_u: Option<f64>,
    /// Lattice shift across the band, in fractions of a cell.
    pub offset_v: Option<f64>,
    /// Peak displacement, mm.
    pub height_mm: Option<f64>,
    /// Flat metal left between neighbouring tiles, mm.
    pub gap_mm: Option<f64>,
    /// Brick offset applied per row, 0 to 1 of a cell.
    pub stagger: Option<f64>,
    /// Mirror every other column.
    pub mirror_alternate_u: Option<bool>,
    /// Mirror every other row.
    pub mirror_alternate_v: Option<bool>,
    /// Gamma on the alpha response. Above 1 deepens, below 1 flattens.
    pub contrast: Option<f64>,
    /// Added to the alpha before shaping, -1 to 1.
    pub bias: Option<f64>,
    /// Invert the alpha.
    pub invert: Option<bool>,
    /// Fade the tiling out over this distance at the `v` edges, mm.
    pub feather_mm: Option<f64>,
    /// Sample the alpha wrapped rather than clamped, so a seamless source keeps
    /// flowing across cell boundaries.
    pub continuous: Option<bool>,
    /// Repeat the strip mirrored about the middle of the section, so one layer
    /// covers both side faces.
    pub mirror_v: Option<bool>,
    /// Snap the strip onto the band's side faces with square cells, overriding
    /// v_center_mm, v_span_mm, rows, repeats_around, feather_mm and mirror_v.
    /// Fails if the profile has no face square to the mould pull.
    pub fit_to_side_faces: Option<bool>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct AddBorderParams {
    /// Layer name in the stack. Defaults to "Border".
    pub name: Option<String>,
    /// Add, Max, Min, Subtract, or Replace. Defaults to Max.
    pub blend: Option<String>,
    pub opacity: Option<f64>,
    /// Centre of the rail across the band, mm of `v`.
    pub v_mm: Option<f64>,
    /// Extent of the rail across the band, mm.
    pub width_mm: Option<f64>,
    /// Peak displacement, mm.
    pub height_mm: Option<f64>,
    /// Round, Flat, Knife, Step, or Rope.
    pub profile: Option<String>,
    /// Place a second rail the same distance from the other edge.
    pub mirror: Option<bool>,
    /// Twists per revolution for the Rope profile. Integer, so it closes.
    pub rope_twists: Option<u32>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct AddSeatPadParams {
    /// Layer name in the stack. Defaults to "Gem seat".
    pub name: Option<String>,
    /// Add, Max, Min, Subtract, or Replace. Defaults to Max.
    pub blend: Option<String>,
    pub opacity: Option<f64>,
    /// Position around the ring, degrees. 90 is the top.
    pub theta_deg: Option<f64>,
    /// Position across the band, mm of `v`.
    pub v_mm: Option<f64>,
    /// The pad's short axis, mm — its diameter when round.
    pub diameter_mm: Option<f64>,
    /// The pad's long axis over its short one, 1 = round. An oval, marquise
    /// or baguette seat should carry its stone's own aspect so the stock
    /// matches the girdle instead of a circle drawn round its length.
    pub elong: Option<f64>,
    /// Turn the pad about its own normal, degrees. 0 lays the long axis
    /// along the ring; 90 stands it across the band.
    pub rot_deg: Option<f64>,
    /// Peak displacement, mm.
    pub height_mm: Option<f64>,
    /// 0 is a flat-topped boss, 1 a full dome.
    pub crown: Option<f64>,
    /// Skirt fairing the pad into the band, mm.
    pub blend_mm: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct AddSignetParams {
    /// Layer name in the stack. Defaults to "Signet".
    pub name: Option<String>,
    /// Add, Max, Min, Subtract, or Replace. Defaults to Max.
    pub blend: Option<String>,
    pub opacity: Option<f64>,
    /// Position around the ring, degrees. 90 is the top.
    pub theta_deg: Option<f64>,
    /// Position across the band, mm of `v`. Defaults to the crest.
    pub v_mm: Option<f64>,
    /// Oval, Round, Cushion, Rectangle, or Hexagon.
    pub outline: Option<String>,
    /// Extent around the ring, mm.
    pub length_mm: Option<f64>,
    /// Extent across the band, mm.
    pub width_mm: Option<f64>,
    /// Height of the table above the band, mm.
    pub height_mm: Option<f64>,
    /// Fraction of the face that stays dead flat, 0..1.
    pub top_flat: Option<f64>,
    /// Grow the table to fill the head, the way a real signet's does. Applied
    /// after the explicit sizes, so it overrides width_mm and length_mm.
    pub fill_head: Option<bool>,
    /// Shoulder fairing the table into the band, mm.
    pub shoulder_mm: Option<f64>,
    /// Rotation of the outline within the band, degrees.
    pub rotation_deg: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct AddMilgrainParams {
    /// Layer name in the stack. Defaults to "Milgrain".
    pub name: Option<String>,
    /// Add, Max, Min, Subtract, or Replace. Defaults to Max.
    pub blend: Option<String>,
    pub opacity: Option<f64>,
    /// Position of the bead line across the band, mm of `v`.
    pub v_mm: Option<f64>,
    pub bead_diameter_mm: Option<f64>,
    /// Beads around the circumference. Integer, so the line closes.
    pub beads_around: Option<u32>,
    /// Peak displacement, mm.
    pub height_mm: Option<f64>,
    /// Repeat the line the same distance from the other edge.
    pub mirror: Option<bool>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct UpdateLayerParams {
    /// Openwork only: metal left standing over the bore, mm.
    pub keep_mm: Option<f64>,
    /// Index in the stack, from list_layers.
    pub index: usize,
    /// Any layer: rename.
    pub name: Option<String>,
    /// Any layer: include it in the composite.
    pub enabled: Option<bool>,
    /// Any layer: Add, Max, Min, Subtract, or Replace.
    pub blend: Option<String>,
    /// Any layer: scale on this layer's output.
    pub opacity: Option<f64>,
    /// Any layer: peak displacement, mm.
    pub height_mm: Option<f64>,
    /// Tiling only.
    pub alpha: Option<String>,
    /// Tiling only.
    pub repeats_around: Option<u32>,
    /// Tiling only.
    pub rows: Option<u32>,
    /// Tiling only.
    pub v_center_mm: Option<f64>,
    /// Tiling only.
    pub v_span_mm: Option<f64>,
    /// Tiling and signet: rotation of the pattern or outline, degrees.
    pub rotation_deg: Option<f64>,
    /// Tiling only.
    pub offset_u: Option<f64>,
    /// Tiling only.
    pub offset_v: Option<f64>,
    /// Tiling only.
    pub gap_mm: Option<f64>,
    /// Tiling only.
    pub stagger: Option<f64>,
    /// Tiling only.
    pub mirror_alternate_u: Option<bool>,
    /// Tiling only.
    pub mirror_alternate_v: Option<bool>,
    /// Tiling only: repeat the strip mirrored about the middle of the section.
    pub mirror_v: Option<bool>,
    /// Tiling only: snap onto the side faces with square cells.
    pub fit_to_side_faces: Option<bool>,
    /// Tiling only.
    pub contrast: Option<f64>,
    /// Tiling only.
    pub bias: Option<f64>,
    /// Tiling only.
    pub invert: Option<bool>,
    /// Tiling only.
    pub feather_mm: Option<f64>,
    /// Tiling only.
    pub continuous: Option<bool>,
    /// Border, gem seat pad, signet, and milgrain: position across the band, mm of `v`.
    pub v_mm: Option<f64>,
    /// Gem seat pad only: long axis over short, 1 = round.
    pub elong: Option<f64>,
    /// Gem seat pad only: turn about the seat normal, degrees.
    pub rot_deg: Option<f64>,
    /// Border and milgrain: repeat on the other side of the band.
    pub mirror: Option<bool>,
    /// Border and signet: extent across the band, mm.
    pub width_mm: Option<f64>,
    /// Border only: Round, Flat, Knife, Step, or Rope.
    pub profile: Option<String>,
    /// Border only: twists per revolution.
    pub rope_twists: Option<u32>,
    /// Gem seat pad and signet: position around the ring, degrees.
    pub theta_deg: Option<f64>,
    /// Gem seat pad only.
    pub diameter_mm: Option<f64>,
    /// Gem seat pad only: 0 flat-topped, 1 full dome.
    pub crown: Option<f64>,
    /// Gem seat pad only: skirt fairing the pad into the band, mm.
    pub blend_mm: Option<f64>,
    /// Milgrain only.
    pub bead_diameter_mm: Option<f64>,
    /// Milgrain only: beads around the circumference.
    pub beads_around: Option<u32>,
    /// Signet only: Oval, Round, Cushion, Rectangle, or Hexagon.
    pub outline: Option<String>,
    /// Signet only: extent around the ring, mm.
    pub length_mm: Option<f64>,
    /// Signet only: fraction of the face that stays dead flat, 0..1.
    pub top_flat: Option<f64>,
    /// Signet only: shoulder fairing the table into the band, mm.
    pub shoulder_mm: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LayerIndexParams {
    /// Index in the stack, from list_layers.
    pub index: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveLayerParams {
    /// Index in the stack, from list_layers.
    pub index: usize,
    /// Steps to move. Negative moves the layer earlier in the composite.
    pub delta: i32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetLayerEnabledParams {
    /// Index in the stack, from list_layers.
    pub index: usize,
    pub enabled: bool,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SetLayerWindowParams {
    /// Index in the stack, from list_layers.
    pub index: usize,
    /// Gate the layer at all. Setting theta_deg, span_deg or fade_deg turns it
    /// on by itself; pass false to go back to running the whole way round.
    pub enabled: Option<bool>,
    /// Ring angle the arc is centred on, degrees. 90 is the top.
    pub theta_deg: Option<f64>,
    /// Arc held at full strength, degrees.
    pub span_deg: Option<f64>,
    /// Falloff at each end of the arc, degrees.
    pub fade_deg: Option<f64>,
    /// Keep the layer everywhere but the arc.
    pub invert: Option<bool>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SectionParams {
    /// Ring angle to slice at, degrees. 90 is the top of the ring.
    pub theta_deg: Option<f64>,
    /// Points to sample the slice at, clamped to 24..4096. Defaults to the
    /// design's profile_steps.
    pub steps: Option<u32>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ExportParams {
    /// Absolute path to write to. Defaults to a temp file.
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PathParams {
    /// Absolute path to the design JSON.
    pub path: String,
}

// --- Helpers ---------------------------------------------------------------

/// Lowercase alphanumerics only, so "Half Round" and "half_round" match.
fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

fn parse_profile_style(s: &str) -> Result<ProfileStyle, ErrorData> {
    let key = norm(s);
    ProfileStyle::ALL
        .iter()
        .copied()
        .find(|v| norm(&format!("{v:?}")) == key || norm(v.label()) == key)
        .ok_or_else(|| {
            let all: Vec<String> = ProfileStyle::ALL.iter().map(|v| format!("{v:?}")).collect();
            ErrorData::invalid_params(
                format!("unknown profile style {s:?}; expected one of {}", all.join(", ")),
                None,
            )
        })
}

fn parse_shank_kind(s: &str) -> Result<ShankKind, ErrorData> {
    let key = norm(s);
    ShankKind::ALL
        .iter()
        .copied()
        .find(|v| norm(&format!("{v:?}")) == key || norm(v.label()) == key)
        .ok_or_else(|| {
            let all: Vec<String> = ShankKind::ALL.iter().map(|v| format!("{v:?}")).collect();
            ErrorData::invalid_params(
                format!("unknown shank kind {s:?}; expected one of {}", all.join(", ")),
                None,
            )
        })
}

fn parse_blend(s: &str) -> Result<Blend, ErrorData> {
    let key = norm(s);
    Blend::ALL
        .iter()
        .copied()
        .find(|v| norm(&format!("{v:?}")) == key || norm(v.label()) == key)
        .ok_or_else(|| {
            ErrorData::invalid_params(
                format!(
                    "unknown blend {s:?}; expected one of Add, Max, Min, Subtract (also accepted as Carve), Replace"
                ),
                None,
            )
        })
}

fn parse_border_profile(s: &str) -> Result<BorderProfile, ErrorData> {
    let key = norm(s);
    BorderProfile::ALL
        .iter()
        .copied()
        .find(|v| norm(&format!("{v:?}")) == key || norm(v.label()) == key)
        .ok_or_else(|| {
            let all: Vec<String> = BorderProfile::ALL.iter().map(|v| format!("{v:?}")).collect();
            ErrorData::invalid_params(
                format!("unknown border profile {s:?}; expected one of {}", all.join(", ")),
                None,
            )
        })
}

fn parse_signet_outline(s: &str) -> Result<SignetOutline, ErrorData> {
    let key = norm(s);
    SignetOutline::ALL
        .iter()
        .copied()
        .find(|v| norm(&format!("{v:?}")) == key || norm(v.label()) == key)
        .ok_or_else(|| {
            let all: Vec<String> = SignetOutline::ALL.iter().map(|v| format!("{v:?}")).collect();
            ErrorData::invalid_params(
                format!("unknown signet outline {s:?}; expected one of {}", all.join(", ")),
                None,
            )
        })
}

fn unknown_alpha(name: &str, lib: &AlphaLibrary) -> ErrorData {
    let key = norm(name);
    let mut near: Vec<&str> = lib
        .iter()
        .map(|a| a.name.as_str())
        .filter(|c| !key.is_empty() && norm(c).contains(&key))
        .take(10)
        .collect();
    if near.is_empty() {
        near = lib.iter().map(|a| a.name.as_str()).take(10).collect();
    }
    ErrorData::invalid_params(
        format!(
            "unknown alpha {name:?}; the library holds {} entries, nearest by name: {}. Call list_alphas for the full list.",
            lib.len(),
            near.join(", ")
        ),
        None,
    )
}

fn bad_index(index: usize, len: usize) -> ErrorData {
    let range = if len == 0 {
        "the stack is empty".to_string()
    } else {
        format!("valid indices are 0..{}", len - 1)
    };
    ErrorData::invalid_params(format!("layer index {index} is out of range; {range}"), None)
}

/// Assign an optional finite value, recording `name=value`.
fn put_f64(
    dst: &mut f64,
    src: Option<f64>,
    name: &str,
    applied: &mut Vec<String>,
) -> Result<(), ErrorData> {
    let Some(v) = src else { return Ok(()) };
    if !v.is_finite() {
        return Err(ErrorData::invalid_params(
            format!("{name} must be a finite number, got {v}"),
            None,
        ));
    }
    *dst = v;
    applied.push(format!("{name}={v}"));
    Ok(())
}

/// Assign an optional finite value inside an inclusive range.
fn put_range(
    dst: &mut f64,
    src: Option<f64>,
    name: &str,
    lo: f64,
    hi: f64,
    applied: &mut Vec<String>,
) -> Result<(), ErrorData> {
    let Some(v) = src else { return Ok(()) };
    if !v.is_finite() || v < lo || v > hi {
        return Err(ErrorData::invalid_params(
            format!("{name} must be between {lo} and {hi}, got {v}"),
            None,
        ));
    }
    *dst = v;
    applied.push(format!("{name}={v}"));
    Ok(())
}

fn put_u32(dst: &mut u32, src: Option<u32>, name: &str, applied: &mut Vec<String>) {
    if let Some(v) = src {
        *dst = v;
        applied.push(format!("{name}={v}"));
    }
}

fn put_bool(dst: &mut bool, src: Option<bool>, name: &str, applied: &mut Vec<String>) {
    if let Some(v) = src {
        *dst = v;
        applied.push(format!("{name}={v}"));
    }
}

fn layer_infos(design: &RingDesign) -> Vec<LayerInfo> {
    design
        .layers
        .layers
        .iter()
        .enumerate()
        .map(|(index, e)| LayerInfo {
            index,
            name: e.name.clone(),
            kind: e.layer.kind_label().to_string(),
            enabled: e.enabled,
            blend: format!("{:?}", e.blend),
            opacity: e.opacity,
            window: e.window.enabled.then(|| {
                format!(
                    "{} {:.0} deg centre, {:.0} deg span, {:.0} deg fade",
                    if e.window.invert { "everywhere but" } else { "only" },
                    e.window.theta_deg,
                    e.window.span_deg,
                    e.window.fade_deg
                )
            }),
        })
        .collect()
}

/// One-line headline of the design.
fn one_line(d: &RingDesign) -> String {
    format!(
        "{} | {} | {} {:.2} x {:.2} mm | {} shank | {} layer(s)",
        d.name,
        d.size.display(),
        d.profile.style.label(),
        d.profile.width_mm,
        d.profile.thickness_mm,
        d.shank.kind.label(),
        d.layers.layers.len()
    )
}

fn metals_json(report: &Report) -> Vec<MetalWeightJson> {
    report
        .metals
        .iter()
        .map(|m| MetalWeightJson { metal: m.metal.to_string(), grams: m.grams, dwt: m.dwt })
        .collect()
}

fn metal_grams(report: &Report, name: &str) -> f64 {
    report.metals.iter().find(|m| m.metal == name).map(|m| m.grams).unwrap_or(0.0)
}

fn report_json(report: &Report, params: BuildParams, generation: u64) -> ReportJson {
    let v = report.validation;
    ReportJson {
        generation,
        watertight: v.watertight,
        triangle_count: v.triangle_count,
        vertex_count: v.vertex_count,
        boundary_edges: v.boundary_edges,
        non_manifold_edges: v.non_manifold_edges,
        theta_steps: params.theta_steps,
        profile_steps: params.profile_steps,
        volume_mm3: report.volume_mm3,
        surface_area_mm2: report.surface_area_mm2,
        bounds_mm: report.bounds_mm,
        inner_diameter_mm: report.inner_diameter_mm,
        outer_diameter_mm: report.outer_diameter_mm,
        band_width_mm: report.band_width_mm,
        max_relief_mm: report.max_relief_mm,
        min_relief_mm: report.min_relief_mm,
        metals: metals_json(report),
        build_ms: report.build_ms as u64,
    }
}

fn stones_json(r: &ringdesign_core::stones::StonesReport) -> StonesJson {
    use ringdesign_core::stones::SeatFooting;
    StonesJson {
        stone_count: r.stone_count,
        total_carats: r.total_carats,
        seats: r
            .seats
            .iter()
            .map(|s| SeatJson {
                label: s.label.clone(),
                style: s.style.label().to_string(),
                count: s.count,
                seat_diameter_mm: s.seat_diameter_mm,
                stone: s.gem.map(|g| g.display()),
                sits_on: match s.footing {
                    SeatFooting::SideFace => "side face".to_string(),
                    SeatFooting::Crown(d) => format!("crown {d:+.1} deg"),
                },
                edge_clearance_mm: s.edge_clearance_mm,
                depth_available_mm: s.depth_available_mm,
                bridge_mm: s.bridge_mm,
                shared_prongs: s.shared_prongs.map(|(pairs, dia, proud)| {
                    format!("{pairs} pairs, {dia:.2} mm posts, {proud:.2} mm proud")
                }),
                warnings: s.warnings.clone(),
            })
            .collect(),
        crowding: r
            .crowding
            .iter()
            .map(|p| StonePairJson {
                a: p.a.clone(),
                b: p.b.clone(),
                a_theta_deg: p.a_theta_deg,
                b_theta_deg: p.b_theta_deg,
                gap_mm: p.gap_mm,
                gap_deep_mm: p.gap_deep_mm,
            })
            .collect(),
        tight_pairs: r.tight_pairs,
    }
}

fn field_json(f: &ringdesign_core::castability::FieldReport) -> FieldJson {
    FieldJson {
        verdict: format!("{:?}", f.verdict),
        verdict_label: f.verdict.label().to_string(),
        worst_draft_deg: f.worst_draft_deg,
        undercut_fraction: f.undercut_fraction(),
        thinnest_wall_mm: f.thinnest_wall_mm,
        thinnest_wall_theta_deg: f.thinnest_wall_theta_deg,
        notes: f.notes.clone(),
    }
}

fn cast_json(
    cast: &CastReport,
    min_draft_deg: f64,
    generation: u64,
    stones: Option<StonesJson>,
    field: FieldJson,
    dfm: Vec<String>,
) -> CastJson {
    CastJson {
        generation,
        verdict: format!("{:?}", cast.verdict),
        verdict_label: cast.verdict.label().to_string(),
        good_faces: cast.good,
        marginal_faces: cast.marginal,
        vertical_faces: cast.vertical,
        undercut_faces: cast.undercut,
        undercut_area_mm2: cast.undercut_area_mm2,
        marginal_area_mm2: cast.marginal_area_mm2,
        total_area_mm2: cast.total_area_mm2,
        undercut_fraction: cast.undercut_fraction(),
        worst_draft_deg: cast.worst_draft_deg,
        parting_z_mm: cast.parting_z_mm,
        min_draft_deg,
        notes: cast.notes.clone(),
        stones,
        field,
        dfm,
    }
}

/// Every `SECTION_POINT_CAP`-th point of the slice, with the summary fields.
fn section_json(section: &Section) -> SectionJson {
    let sampled = section.points.len();
    let stride = sampled.div_ceil(SECTION_POINT_CAP).max(1);
    let points: Vec<SectionPointJson> = section
        .points
        .iter()
        .step_by(stride)
        .map(|p| SectionPointJson {
            r: p.r,
            z: p.z,
            draft_deg: p.draft_deg,
            class: p.class.label().to_string(),
            surface: p.surface,
        })
        .collect();
    SectionJson {
        theta_deg: section.theta_deg,
        parting_z_mm: section.parting_z_mm,
        min_r_mm: section.min_r,
        max_r_mm: section.max_r,
        min_z_mm: section.min_z,
        max_z_mm: section.max_z,
        min_wall_mm: section.min_wall_mm,
        undercut_count: section.undercut_count,
        sampled_points: sampled,
        returned_points: points.len(),
        points,
    }
}

/// Filesystem-safe stem for a default export path.
fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "ring".into() } else { s }
}

fn default_export_path(name: &str, ext: &str) -> String {
    std::env::temp_dir()
        .join(format!("{}.{ext}", slug(name)))
        .to_string_lossy()
        .into_owned()
}

/// Names of the kind-specific `update_layer` fields the call actually set.
fn kind_specific_present(p: &UpdateLayerParams) -> Vec<&'static str> {
    let flags: [(&'static str, bool); 36] = [
        ("alpha", p.alpha.is_some()),
        ("repeats_around", p.repeats_around.is_some()),
        ("rows", p.rows.is_some()),
        ("v_center_mm", p.v_center_mm.is_some()),
        ("v_span_mm", p.v_span_mm.is_some()),
        ("rotation_deg", p.rotation_deg.is_some()),
        ("offset_u", p.offset_u.is_some()),
        ("offset_v", p.offset_v.is_some()),
        ("gap_mm", p.gap_mm.is_some()),
        ("stagger", p.stagger.is_some()),
        ("mirror_alternate_u", p.mirror_alternate_u.is_some()),
        ("mirror_alternate_v", p.mirror_alternate_v.is_some()),
        ("mirror_v", p.mirror_v.is_some()),
        ("fit_to_side_faces", p.fit_to_side_faces.is_some()),
        ("contrast", p.contrast.is_some()),
        ("bias", p.bias.is_some()),
        ("invert", p.invert.is_some()),
        ("feather_mm", p.feather_mm.is_some()),
        ("continuous", p.continuous.is_some()),
        ("v_mm", p.v_mm.is_some()),
        ("mirror", p.mirror.is_some()),
        ("width_mm", p.width_mm.is_some()),
        ("profile", p.profile.is_some()),
        ("rope_twists", p.rope_twists.is_some()),
        ("theta_deg", p.theta_deg.is_some()),
        ("diameter_mm", p.diameter_mm.is_some()),
        ("elong", p.elong.is_some()),
        ("rot_deg", p.rot_deg.is_some()),
        ("crown", p.crown.is_some()),
        ("blend_mm", p.blend_mm.is_some()),
        ("bead_diameter_mm", p.bead_diameter_mm.is_some()),
        ("beads_around", p.beads_around.is_some()),
        ("outline", p.outline.is_some()),
        ("length_mm", p.length_mm.is_some()),
        ("top_flat", p.top_flat.is_some()),
        ("shoulder_mm", p.shoulder_mm.is_some()),
    ];
    flags.iter().filter(|(_, set)| *set).map(|(name, _)| *name).collect()
}

const TILING_FIELDS: &[&str] = &[
    "alpha",
    "repeats_around",
    "rows",
    "v_center_mm",
    "v_span_mm",
    "rotation_deg",
    "offset_u",
    "offset_v",
    "gap_mm",
    "stagger",
    "mirror_alternate_u",
    "mirror_alternate_v",
    "mirror_v",
    "fit_to_side_faces",
    "contrast",
    "bias",
    "invert",
    "feather_mm",
    "continuous",
];
const BORDER_FIELDS: &[&str] = &["v_mm", "width_mm", "profile", "mirror", "rope_twists"];
const SEAT_PAD_FIELDS: &[&str] =
    &["theta_deg", "v_mm", "diameter_mm", "elong", "rot_deg", "crown", "blend_mm"];
const MILGRAIN_FIELDS: &[&str] = &["v_mm", "bead_diameter_mm", "beads_around", "mirror"];
const SIGNET_FIELDS: &[&str] = &[
    "theta_deg",
    "v_mm",
    "outline",
    "length_mm",
    "width_mm",
    "top_flat",
    "shoulder_mm",
    "rotation_deg",
];

fn allowed_fields(layer: &Layer) -> &'static [&'static str] {
    match layer {
        Layer::Tiling(_) => TILING_FIELDS,
        Layer::Signet(_) => SIGNET_FIELDS,
        Layer::Border(_) => BORDER_FIELDS,
        Layer::SeatPad(_) => SEAT_PAD_FIELDS,
        Layer::Milgrain(_) => MILGRAIN_FIELDS,
        // Groups carry only the common entry fields; edit their children by index.
        Layer::Group(_) => &[],
        Layer::Curve(_) => CURVE_FIELDS,
        Layer::Flutes(_) => FLUTES_FIELDS,
        Layer::Decals(_) => DECAL_FIELDS,
        Layer::SeatRun(_) => SEAT_RUN_FIELDS,
        Layer::Openwork(_) => OPENWORK_FIELDS,
    }
}

const OPENWORK_FIELDS: &[&str] = &["alpha", "repeats_around", "v_center_mm", "v_span_mm", "keep_mm"];

const SEAT_RUN_FIELDS: &[&str] = &["repeats_around", "v_mm", "height_mm"];

const DECAL_FIELDS: &[&str] = &["alpha", "height_mm"];

const FLUTES_FIELDS: &[&str] = &["repeats_around", "width_mm", "height_mm"];

const CURVE_FIELDS: &[&str] = &["repeats_around", "width_mm", "height_mm"];

// --- Tools -----------------------------------------------------------------

#[tool_router]
impl RingDesignServer {
    #[tool(
        description = "Return the complete RingDesign as JSON (name, size, profile, shank, every layer, build resolution, casting settings) plus the engine generation counter. Use describe_ring first for a readable overview; use this when you need exact stored values or want to diff before and after an edit. The generation increments on every mutation, including edits made in a GUI sharing this engine."
    )]
    async fn get_design(&self) -> Result<Json<DesignJson>, ErrorData> {
        let e = self.engine.lock();
        let generation = e.generation();
        let design = serde_json::to_value(e.design())
            .map_err(|err| ErrorData::internal_error(format!("serialize design: {err}"), None))?;
        drop(e);
        Ok(Json(DesignJson { generation, design }))
    }

    #[tool(
        description = "Compact overview of the current ring, and the first thing to read: name, US size and bore diameter, profile style with its sand-casting note, cross-section dimensions in mm, shank style, the unrolled band extents every layer is positioned in (u = arc around the ring, wraps at the circumference; v = arc across the cross-section from one bore edge over the outer surface to the other), the crest position in v, the side faces — the v runs square enough to the mould pull to hold deep relief, null on a dome — the layer stack, and headline output — outer diameter, band width, volume, peak relief and cast weight in sterling silver and 14k gold. Rebuilds the mesh if the design changed since the last build."
    )]
    async fn describe_ring(&self) -> Json<RingSummary> {
        let mut e = self.engine.lock();
        let report = e.report();
        let generation = e.generation();
        let d = e.design();
        let ctx = d.field_context();
        let summary = RingSummary {
            generation,
            name: d.name.clone(),
            size_us: d.size.0,
            size_label: d.size.display(),
            inner_diameter_mm: d.size.inner_diameter_mm(),
            profile_style: format!("{:?}", d.profile.style),
            profile_casting_note: d.profile.style.casting_note().to_string(),
            width_mm: d.profile.width_mm,
            thickness_mm: d.profile.thickness_mm,
            crown_mm: d.profile.effective_crown_mm(),
            edge_thickness_mm: d.profile.edge_thickness_mm(),
            comfort_fit_mm: d.profile.comfort_fit_mm,
            side_draft_deg: d.profile.side_draft_deg,
            shank: format!("{:?}", d.shank.kind),
            shank_amount: d.shank.amount,
            circumference_mm: ctx.circumference_mm,
            band_v_len_mm: ctx.band_v_len_mm,
            crest_v_mm: ctx.crest_v_mm,
            side_faces: ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG).map(|f| SideFaceInfo {
                low_mm: f.low.map(|(a, b)| [a, b]),
                high_mm: f.high.map(|(a, b)| [a, b]),
                even: f.is_even(),
                min_draft_deg: SIDE_FACE_MIN_DRAFT_DEG,
            }),
            layers: layer_infos(d),
            outer_diameter_mm: report.outer_diameter_mm,
            band_width_mm: report.band_width_mm,
            volume_mm3: report.volume_mm3,
            max_relief_mm: report.max_relief_mm,
            min_relief_mm: report.min_relief_mm,
            silver_925_g: metal_grams(&report, "Silver 925"),
            gold_14k_g: metal_grams(&report, "Gold 14k"),
            triangle_count: report.validation.triangle_count,
            watertight: report.validation.watertight,
            summary: one_line(d),
        };
        drop(e);
        self.touch();
        Json(summary)
    }

    #[tool(
        description = "Every band cross-section preset with its label, its superellipse parameters, and one line on how it behaves in a two-part sand mould. The mould parts on a plane perpendicular to the finger axis and pulls both ways, so the outer surface must drop monotonically from a single crest; every style does, which is why the bare profile can never undercut. What the style decides is how much draft the flanks carry: Flat and LowDome leave a near-vertical wall that will not hold relief, HalfRound and HighDome give the crown real slope and are the best carriers for carved pattern."
    )]
    async fn list_profile_styles(&self) -> Json<ProfileStyleList> {
        let styles = ProfileStyle::ALL
            .iter()
            .map(|s| {
                let (a, b, crown, round) = s.preset();
                ProfileStyleInfo {
                    style: format!("{s:?}"),
                    label: s.label().to_string(),
                    casting_note: s.casting_note().to_string(),
                    shape_a: a,
                    shape_b: b,
                    crown_fraction: crown,
                    edge_round_fraction: round,
                }
            })
            .collect();
        Json(ProfileStyleList { styles })
    }

    #[tool(
        description = "Every shank style with its label and description. The shank scales the cross-section per ring angle (90 degrees is the top of the ring): Tapered narrows toward the palm, ReverseTaper the opposite, Cathedral swells the shoulders either side of the top, EuroFlat cuts a flat chord across the bottom so the ring will not spin. Layers are evaluated against the unmodulated reference cross-section, so a pattern follows the band as it tapers instead of sliding across it."
    )]
    async fn list_shank_styles(&self) -> Json<ShankStyleList> {
        let kinds = ShankKind::ALL
            .iter()
            .map(|k| ShankStyleInfo {
                kind: format!("{k:?}"),
                label: k.label().to_string(),
                description: k.description().to_string(),
            })
            .collect();
        Json(ShankStyleList { kinds })
    }

    #[tool(
        description = "List the grayscale height-map alphas a tiling layer can use, with their pixel dimensions. `filter` keeps only names containing that substring (case-insensitive); `limit` caps the rows returned (default 100, maximum 500) so a large library does not flood the context — `matched` reports how many the filter actually found. An alpha samples to 0..1 and is multiplied by the layer's height_mm, so white is full displacement outward along the surface normal and black leaves the base surface alone. Built-in patterns are periodic in both axes and imported images are cross-faded, so a tile butts against its neighbour without a seam."
    )]
    async fn list_alphas(&self, Parameters(p): Parameters<ListAlphasParams>) -> Json<AlphaList> {
        let limit = p.limit.unwrap_or(ALPHA_LIMIT_DEFAULT).clamp(1, ALPHA_LIMIT_MAX) as usize;
        let filter = p.filter.unwrap_or_default().to_lowercase();
        let e = self.engine.lock();
        let lib = e.library();
        let total = lib.len();
        let matching: Vec<&ringdesign_core::Alpha> = lib
            .iter()
            .filter(|a| filter.is_empty() || a.name.to_lowercase().contains(&filter))
            .collect();
        let matched = matching.len();
        let alphas: Vec<AlphaInfo> = matching
            .into_iter()
            .take(limit)
            .map(|a| AlphaInfo { name: a.name.clone(), width: a.width, height: a.height })
            .collect();
        drop(e);
        Json(AlphaList { total, matched, returned: alphas.len(), alphas })
    }

    #[tool(
        description = "The layer stack in composite order: index, name, kind, enabled, blend mode and opacity. Layer 0 is applied first and each layer's height is scaled by its opacity then folded into the running total by its blend — Add sums, Max keeps the taller, Min the shorter, Subtract carves the shape into what is under it, Replace discards everything under it. Indices from this list are what update_layer, remove_layer, move_layer and set_layer_enabled take."
    )]
    async fn list_layers(&self) -> Json<LayerList> {
        let e = self.engine.lock();
        let layers = layer_infos(e.design());
        drop(e);
        Json(LayerList { count: layers.len(), layers })
    }

    #[tool(
        description = "Set the design name and/or the US finger size; an omitted field keeps its value. Size is rounded to the nearest quarter (US 7 is a 17.35 mm bore, 54.5 mm inner circumference). Size changes the circumference, so anything positioned by an integer count around the ring — tiling repeats_around, milgrain beads_around, border rope_twists — stays seamless but its cells get wider or narrower."
    )]
    async fn set_ring(
        &self,
        Parameters(p): Parameters<SetRingParams>,
    ) -> Result<Json<DesignChange>, ErrorData> {
        if let Some(size) = p.size
            && (!size.is_finite() || !(0.5..=20.0).contains(&size))
        {
            return Err(ErrorData::invalid_params(
                format!("size must be a US size between 0.5 and 20, got {size}"),
                None,
            ));
        }
        let mut applied = Vec::new();
        let mut e = self.engine.lock();
        let d = e.design_mut();
        if let Some(name) = p.name {
            applied.push(format!("name={name}"));
            d.name = name;
        }
        if let Some(size) = p.size {
            d.size = RingSize::new(size);
            applied.push(format!("size={}", d.size.0));
        }
        let change = DesignChange {
            generation: e.generation(),
            applied,
            summary: one_line(e.design()),
        };
        drop(e);
        self.touch();
        Ok(Json(change))
    }

    #[tool(
        description = "Update the band cross-section; every field is optional and an omitted one keeps its value. `style` applies a preset and OVERWRITES shape_a, shape_b, crown_mm and edge_round_mm — it is applied first, so anything else you pass in the same call wins over the preset. Passing style together with thickness_mm re-derives crown_mm and edge_round_mm from the new thickness unless you also pass them. Units, all mm unless stated: width_mm is the axial extent at the bore, thickness_mm the maximum radial thickness at the crest, crown_mm the radial drop from crest to outer edge (clamped so the edge keeps 0.2 mm — a feather edge will not fill), edge_round_mm the fillet where the side faces meet the outer surface, comfort_fit_mm the inward dome of the bore (the stated size is measured at its crown, so the ring rides on a narrow contact band), side_draft_deg the taper of the side faces in degrees (positive narrows the band outward and adds draft). shape_a and shape_b are the superellipse exponents in drop(x) = 1 - (1 - x^a)^(1/b): higher a flattens the crown and sharpens the edge falloff, higher b fills the crest out. crest_bias slides the crest across the width from -1 at one edge to 1 at the other. The drop is monotonic for any a, b > 0, so the bare profile never undercuts a two-part pull — but a taller crown steepens the flanks, and steep flanks are what make relief castable."
    )]
    async fn set_profile(
        &self,
        Parameters(p): Parameters<SetProfileParams>,
    ) -> Result<Json<DesignChange>, ErrorData> {
        let style = p.style.as_deref().map(parse_profile_style).transpose()?;
        let mut applied = Vec::new();
        let mut e = self.engine.lock();
        let d = e.design_mut();
        if let Some(style) = style {
            d.profile.apply_style(style);
            applied.push(format!("style={style:?}"));
        }
        put_f64(&mut d.profile.width_mm, p.width_mm, "width_mm", &mut applied)?;
        put_f64(&mut d.profile.thickness_mm, p.thickness_mm, "thickness_mm", &mut applied)?;
        // Re-derive the preset's crown and fillet from a thickness set here.
        if let (Some(style), Some(_)) = (style, p.thickness_mm) {
            let (_, _, crown_frac, round_frac) = style.preset();
            if style != ProfileStyle::Custom {
                if p.crown_mm.is_none() {
                    d.profile.crown_mm = d.profile.thickness_mm * crown_frac;
                }
                if p.edge_round_mm.is_none() {
                    d.profile.edge_round_mm = d.profile.thickness_mm * round_frac;
                }
            }
        }
        put_f64(&mut d.profile.crown_mm, p.crown_mm, "crown_mm", &mut applied)?;
        put_range(&mut d.profile.shape_a, p.shape_a, "shape_a", 0.05, 16.0, &mut applied)?;
        put_range(&mut d.profile.shape_b, p.shape_b, "shape_b", 0.05, 16.0, &mut applied)?;
        put_range(&mut d.profile.crest_bias, p.crest_bias, "crest_bias", -1.0, 1.0, &mut applied)?;
        put_f64(&mut d.profile.edge_round_mm, p.edge_round_mm, "edge_round_mm", &mut applied)?;
        put_f64(&mut d.profile.comfort_fit_mm, p.comfort_fit_mm, "comfort_fit_mm", &mut applied)?;
        put_range(
            &mut d.profile.side_draft_deg,
            p.side_draft_deg,
            "side_draft_deg",
            -20.0,
            30.0,
            &mut applied,
        )?;
        let change = DesignChange {
            generation: e.generation(),
            applied,
            summary: one_line(e.design()),
        };
        drop(e);
        self.touch();
        Ok(Json(change))
    }

    #[tool(
        description = "Set the shank style and how hard it modulates. `kind` is Uniform, Tapered, ReverseTaper, Cathedral, EuroFlat, or Signet (see list_shank_styles); `amount` is 0 to 1, where 0 is no modulation. The shank scales the cross-section per ring angle — a tapered shank at amount 1 loses 45% of its width at the bottom of the finger. It does not move the height field: layers stay parameterized against the unmodulated cross-section, so a pattern narrows with the band instead of running off it. Signet is different in kind: it makes a head out of the band itself rather than adding anything to it. head_outline is the plan silhouette the band's width follows, head_length_mm the extent of the face around the ring (its extent across the band is the profile's width_mm, so set that to the head), head_rise_mm how far the table stands above the crest, head_shoulder_deg the arc the crest takes to fall back to the shank, head_swell_deg the much longer arc the band's *width* takes to come back to the shank, head_body_fair how far the body under the table rounds away from the face's outline, and head_table_flat 1 for a true plane to engrave. Two of those are easy to get wrong. The swell is the thing a signet reads as from the side and it is not the face: measured on a real 14.7 mm signet the face runs out at 31 degrees off the top while the body keeps widening to 75, so leave head_swell_deg near its 75 default unless you want a stubbier or longer sweep. And the face is a facet cut across a wider body, not a shape extruded down to the finger: at head_body_fair 0 a heart's dimple runs the whole depth of the ring and its lobes leave a crease down each flank, so leave it at 1 unless a prism is what you want. Do not add a signet table layer on top of this — the table is already the band's own crest. Castability: measured 0.000% undercut on every outline, because a band that widens and rises toward the top is single-valued in Z over (r, theta) and so releases by construction. The flat table is a vertical wall with respect to a +/-Z pull, which is fine — it is blank and hand-engraved, and design goes on the head's flanks, which face the pull."
    )]
    async fn set_shank(
        &self,
        Parameters(p): Parameters<SetShankParams>,
    ) -> Result<Json<DesignChange>, ErrorData> {
        let kind = p.kind.as_deref().map(parse_shank_kind).transpose()?;
        let outline = p.head_outline.as_deref().map(parse_signet_outline).transpose()?;
        let mut applied = Vec::new();
        let mut e = self.engine.lock();
        let d = e.design_mut();
        if let Some(kind) = kind {
            d.shank.kind = kind;
            applied.push(format!("kind={kind:?}"));
        }
        put_range(&mut d.shank.amount, p.amount, "amount", 0.0, 1.0, &mut applied)?;
        if let Some(outline) = outline {
            d.shank.head.outline = outline;
            // Sized to the shape unless the call says otherwise, so an outline
            // arrives as that shape rather than the last one restretched.
            if p.head_length_mm.is_none() {
                let width = d.profile.width_mm;
                d.shank.head.fit_length_to(width);
            }
            applied.push(format!("head_outline={outline:?}"));
        }
        let h = &mut d.shank.head;
        put_range(&mut h.length_mm, p.head_length_mm, "head_length_mm", 2.0, 40.0, &mut applied)?;
        put_range(&mut h.rise_mm, p.head_rise_mm, "head_rise_mm", 0.0, 8.0, &mut applied)?;
        put_range(&mut h.shoulder_deg, p.head_shoulder_deg, "head_shoulder_deg", 5.0, 120.0, &mut applied)?;
        if let Some(v) = p.head_swell_deg {
            // Zero hands it back to the head's own proportions, which is where
            // it comes from unless something says otherwise.
            if v <= 0.0 {
                h.swell_deg = None;
                applied.push("head_swell_deg=auto".into());
            } else {
                let mut set = h.swell_deg.unwrap_or(v);
                put_range(&mut set, Some(v), "head_swell_deg", 10.0, 170.0, &mut applied)?;
                h.swell_deg = Some(set);
            }
        }
        put_range(&mut h.body_fair, p.head_body_fair, "head_body_fair", 0.0, 1.0, &mut applied)?;
        put_range(&mut h.table_flat, p.head_table_flat, "head_table_flat", 0.0, 1.0, &mut applied)?;
        put_range(&mut h.rim_round_mm, p.head_rim_round_mm, "head_rim_round_mm", 0.0, 2.0, &mut applied)?;
        put_range(&mut h.theta_deg, p.head_theta_deg, "head_theta_deg", 0.0, 360.0, &mut applied)?;
        if let Some(o) = p.second_head_outline.as_deref() {
            if o.eq_ignore_ascii_case("none") {
                d.shank.extra_heads.clear();
                applied.push("second_head=removed".into());
            } else {
                let outline = parse_signet_outline(o)?;
                if d.shank.extra_heads.is_empty() {
                    let primary_theta = d.shank.head.theta_deg;
                    d.shank.extra_heads.push(ringdesign_core::profile::SignetHead {
                        theta_deg: primary_theta + 48.0,
                        ..Default::default()
                    });
                }
                let h2 = &mut d.shank.extra_heads[0];
                h2.outline = outline;
                h2.fit_length_to(d.profile.width_mm * 0.8);
                applied.push(format!("second_head_outline={outline:?}"));
            }
        }
        if let Some(h2) = d.shank.extra_heads.first_mut() {
            put_range(&mut h2.length_mm, p.second_head_length_mm, "second_head_length_mm", 2.0, 40.0, &mut applied)?;
            put_range(&mut h2.theta_deg, p.second_head_theta_deg, "second_head_theta_deg", 0.0, 360.0, &mut applied)?;
        }
        let change = DesignChange {
            generation: e.generation(),
            applied,
            summary: one_line(e.design()),
        };
        drop(e);
        self.touch();
        Ok(Json(change))
    }

    #[tool(
        description = "Set how the casting analysis is run; this changes what castability reports, never the geometry. parting_z_mm is the height of the parting plane in mm — the cope pulls +Z off everything above it, the drag -Z off everything below. auto_parting puts that plane at the widest silhouette of the ring and ignores parting_z_mm; turn it off only to test a specific mould split. min_draft_deg is the angle below which a wall is called marginal and will drag on the sand; 3 degrees is normal for Delft clay or petrobond. min_section_mm is the thinnest section expected to fill."
    )]
    async fn set_casting(
        &self,
        Parameters(p): Parameters<SetCastingParams>,
    ) -> Result<Json<DesignChange>, ErrorData> {
        let mut applied = Vec::new();
        let mut e = self.engine.lock();
        let d = e.design_mut();
        put_f64(&mut d.draft.parting_z_mm, p.parting_z_mm, "parting_z_mm", &mut applied)?;
        put_range(&mut d.draft.min_draft_deg, p.min_draft_deg, "min_draft_deg", 0.0, 45.0, &mut applied)?;
        put_bool(&mut d.draft.auto_parting, p.auto_parting, "auto_parting", &mut applied);
        put_range(&mut d.draft.min_section_mm, p.min_section_mm, "min_section_mm", 0.05, 10.0, &mut applied)?;
        let change = DesignChange {
            generation: e.generation(),
            applied,
            summary: one_line(e.design()),
        };
        drop(e);
        self.touch();
        Ok(Json(change))
    }

    #[tool(
        description = "Set the mesh resolution stored in the design. theta_steps is sweep steps around the ring, clamped to 24..4096 at build time; profile_steps is vertices around the cross-section, clamped to 24..1024. Triangles = theta_steps * profile_steps * 2, so 512 x 192 is about 197k. Use 192 x 96 while iterating and 1024 x 320 for export. Fine relief needs enough steps that each feature spans several vertices in both directions, otherwise it is silently smoothed away and the castability report will look better than the real casting."
    )]
    async fn set_build_params(
        &self,
        Parameters(p): Parameters<SetBuildParamsParams>,
    ) -> Result<Json<DesignChange>, ErrorData> {
        let mut applied = Vec::new();
        let mut e = self.engine.lock();
        let d = e.design_mut();
        if let Some(v) = p.theta_steps {
            d.build.theta_steps = (v as usize).clamp(24, 4096);
            applied.push(format!("theta_steps={}", d.build.theta_steps));
        }
        if let Some(v) = p.profile_steps {
            d.build.profile_steps = (v as usize).clamp(24, 1024);
            applied.push(format!("profile_steps={}", d.build.profile_steps));
        }
        let change = DesignChange {
            generation: e.generation(),
            applied,
            summary: one_line(e.design()),
        };
        drop(e);
        self.touch();
        Ok(Json(change))
    }

    #[tool(
        description = "Add a layer that tiles an alpha height map around the band. `alpha` names an entry from list_alphas. repeats_around is how many tiles go around the circumference and is an INTEGER, so the lattice divides the circumference exactly and the pattern closes on itself with no seam at 0 degrees. rows stacks tiles across the band. v_center_mm and v_span_mm place the tiled strip across the cross-section in mm of v (v = 0 is one bore edge, the total span and the crest position come from describe_ring). height_mm is the peak displacement along the surface normal. Castability: the crest line is tangent to the parting plane, so relief there undercuts almost immediately — about 0.05 mm is all a half-round band will take on the crest, while the same texture on the flanks a millimetre or two either side casts cleanly. Centring a tall pattern on the crest is the single most common way to make a ring unmouldable. The side faces are the opposite case: on a face square to the mould pull, relief moves along the pull and the walls it raises are parallel to it, so it cannot undercut at any height — measured clean to 1.6 mm where the same tiles on the crest fail at 0.3 mm. Pass fit_to_side_faces=true to snap the strip onto them with unstretched cells; describe_ring reports whether the profile has any (a dome does not — square the sides with set_profile side_draft_deg=0 and a small edge_round_mm, or add an edge flange). mirror_v repeats the strip on the far side so one layer decorates both. Defaults: 24 repeats, 1 row, centred on the crest, spanning 60% of the band, 0.35 mm high, feathered 0.4 mm at the strip edges, blended Max, sampled wrapped so a seamless source keeps flowing across cell boundaries."
    )]
    async fn add_tiling_layer(
        &self,
        Parameters(p): Parameters<AddTilingParams>,
    ) -> Result<Json<LayerChange>, ErrorData> {
        let blend = p.blend.as_deref().map(parse_blend).transpose()?;
        let mut applied = Vec::new();
        let mut e = self.engine.lock();
        if e.library().get(&p.alpha).is_none() {
            let err = unknown_alpha(&p.alpha, e.library());
            drop(e);
            return Err(err);
        }
        let ctx = e.design().field_context();
        let mut t = TilingLayer::default_for(p.alpha.clone(), &ctx);
        put_u32(&mut t.repeats_around, p.repeats_around, "repeats_around", &mut applied);
        put_u32(&mut t.rows, p.rows, "rows", &mut applied);
        put_f64(&mut t.v_center_mm, p.v_center_mm, "v_center_mm", &mut applied)?;
        put_f64(&mut t.v_span_mm, p.v_span_mm, "v_span_mm", &mut applied)?;
        put_f64(&mut t.rotation_deg, p.rotation_deg, "rotation_deg", &mut applied)?;
        put_f64(&mut t.offset_u, p.offset_u, "offset_u", &mut applied)?;
        put_f64(&mut t.offset_v, p.offset_v, "offset_v", &mut applied)?;
        put_f64(&mut t.height_mm, p.height_mm, "height_mm", &mut applied)?;
        put_f64(&mut t.gap_mm, p.gap_mm, "gap_mm", &mut applied)?;
        put_f64(&mut t.stagger, p.stagger, "stagger", &mut applied)?;
        put_bool(&mut t.mirror_alternate_u, p.mirror_alternate_u, "mirror_alternate_u", &mut applied);
        put_bool(&mut t.mirror_alternate_v, p.mirror_alternate_v, "mirror_alternate_v", &mut applied);
        put_range(&mut t.contrast, p.contrast, "contrast", 0.05, 8.0, &mut applied)?;
        put_range(&mut t.bias, p.bias, "bias", -1.0, 1.0, &mut applied)?;
        put_bool(&mut t.invert, p.invert, "invert", &mut applied);
        put_f64(&mut t.feather_mm, p.feather_mm, "feather_mm", &mut applied)?;
        put_bool(&mut t.continuous, p.continuous, "continuous", &mut applied);
        put_bool(&mut t.mirror_v, p.mirror_v, "mirror_v", &mut applied);
        if p.fit_to_side_faces == Some(true) {
            fit_sides(&mut t, &ctx, &mut applied)?;
        }

        let name = p.name.unwrap_or_else(|| p.alpha.clone());
        let mut entry = LayerEntry::new(name, Layer::Tiling(t));
        apply_entry_common(&mut entry, blend, p.opacity, &mut applied)?;
        let change = push_layer(&mut e, entry, applied);
        drop(e);
        self.touch();
        Ok(Json(change))
    }

    #[tool(
        description = "Add a rail running the full way around the band at a fixed position across it. v_mm is the centre of the rail in mm of v (0 is one bore edge), width_mm its extent across the band, height_mm its peak displacement. `mirror` places a second rail the same distance in from the other edge, which is the usual way to frame a tiled centre strip. Profiles: Round (half-round wire), Flat (flat rail with rounded shoulders), Knife (triangular), Step (flat top with a sharp shoulder), Rope (a round rail whose bead spirals). rope_twists is twists per revolution and is an INTEGER, so the twist closes on itself at 0 degrees. Castability: rails belong on the flanks near the edges, where the profile already carries draft. A rail sitting on the crest line adds a wall that leans back under a two-part pull, and a Step or Knife rail anywhere steep is the fastest way to a locked mould — check castability after adding one. Defaults: v_mm 1.0, 0.7 mm wide, 0.35 mm high, Round, mirrored, 48 twists."
    )]
    async fn add_border_layer(
        &self,
        Parameters(p): Parameters<AddBorderParams>,
    ) -> Result<Json<LayerChange>, ErrorData> {
        let blend = p.blend.as_deref().map(parse_blend).transpose()?;
        let profile = p.profile.as_deref().map(parse_border_profile).transpose()?;
        let mut applied = Vec::new();
        let mut b = BorderLayer::default();
        put_f64(&mut b.v_mm, p.v_mm, "v_mm", &mut applied)?;
        put_f64(&mut b.width_mm, p.width_mm, "width_mm", &mut applied)?;
        put_f64(&mut b.height_mm, p.height_mm, "height_mm", &mut applied)?;
        if let Some(profile) = profile {
            b.profile = profile;
            applied.push(format!("profile={profile:?}"));
        }
        put_bool(&mut b.mirror, p.mirror, "mirror", &mut applied);
        put_u32(&mut b.rope_twists, p.rope_twists, "rope_twists", &mut applied);

        let name = p.name.unwrap_or_else(|| "Border".to_string());
        let mut entry = LayerEntry::new(name, Layer::Border(b));
        apply_entry_common(&mut entry, blend, p.opacity, &mut applied)?;
        let mut e = self.engine.lock();
        let change = push_layer(&mut e, entry, applied);
        drop(e);
        self.touch();
        Ok(Json(change))
    }

    #[tool(
        description = "Add a raised circular boss for a bench jeweller to cut a stone seat into. theta_deg positions it around the ring (90 degrees is the top), v_mm across the band in mm of v, diameter_mm and height_mm size it. `crown` runs 0 for a flat-topped boss to 1 for a full dome; blend_mm is the skirt that fairs a flat-topped pad back into the band. Castability: a flat-topped pad has straight walls that undercut from every angle — keep crown at or near 1 unless you want a boss you will file to shape by hand, and put the pad on the crest so both halves of the mould pull away from it. The pad seats a stone roughly diameter_mm - 1.2 across. For an elongated stone set `elong` to its length over its width so the stock follows the girdle instead of a circle drawn round its length, and `rot_deg` to turn it — 0 lays it along the ring, 90 across the band. Defaults: at the top of the ring, 5 mm across, 1.2 mm tall, crown 0.65, 0.8 mm skirt."
    )]
    async fn add_seat_pad_layer(
        &self,
        Parameters(p): Parameters<AddSeatPadParams>,
    ) -> Result<Json<LayerChange>, ErrorData> {
        let blend = p.blend.as_deref().map(parse_blend).transpose()?;
        let mut applied = Vec::new();
        let mut s = SeatPadLayer::default();
        put_f64(&mut s.theta_deg, p.theta_deg, "theta_deg", &mut applied)?;
        put_f64(&mut s.v_mm, p.v_mm, "v_mm", &mut applied)?;
        put_f64(&mut s.diameter_mm, p.diameter_mm, "diameter_mm", &mut applied)?;
        put_range(&mut s.elong, p.elong, "elong", 1.0, 8.0, &mut applied)?;
        put_f64(&mut s.rot_deg, p.rot_deg, "rot_deg", &mut applied)?;
        put_f64(&mut s.height_mm, p.height_mm, "height_mm", &mut applied)?;
        put_range(&mut s.crown, p.crown, "crown", 0.0, 1.0, &mut applied)?;
        put_f64(&mut s.blend_mm, p.blend_mm, "blend_mm", &mut applied)?;

        let name = p.name.unwrap_or_else(|| "Gem seat".to_string());
        let mut entry = LayerEntry::new(name, Layer::SeatPad(s));
        apply_entry_common(&mut entry, blend, p.opacity, &mut applied)?;
        let mut e = self.engine.lock();
        // The pad defaults to v = 0; centre it on the crest when unspecified.
        if p.v_mm.is_none()
            && let Layer::SeatPad(s) = &mut entry.layer
        {
            s.v_mm = e.design().field_context().crest_v_mm;
            applied.push(format!("v_mm={:.3} (crest)", s.v_mm));
        }
        let change = push_layer(&mut e, entry, applied);
        drop(e);
        self.touch();
        Ok(Json(change))
    }

    #[tool(
        description = "Add a raised flat table pad standing on the band: a face for a bench engraver to cut by hand, faired back down into the band. THIS IS NOT HOW TO MAKE A SIGNET — a signet's head is the band's own swell, so use set_shank with kind=Signet and a head_outline, which shapes the ring itself and blends into the shank. This pad sits on top of whatever is under it, which is right for a flat facet on an ordinary band and wrong for a signet, where it leaves a disc glued to a ring. theta_deg positions it around the ring (90 degrees is the top), v_mm across the band in mm of v, and it defaults to the crest so both halves of the mould pull away from the face. length_mm is its extent around the ring, width_mm across the band, height_mm how far the table stands above the band. Outlines: Oval and Round (Round is a true circle on the smaller extent), Cushion, Rectangle, Hexagon. top_flat is the fraction of the face that stays dead flat before the roll-off starts, so keep it high — the flat is the engraving area and a domed table fights the graver. shoulder_mm is the fairing that takes the table back down to the band instead of leaving a vertical wall. rotation_deg turns the outline within the band. Castability: a flat top facing the pull has perfect draft, and the shoulder is what keeps the sides from standing vertical — a shoulder near zero is an undercut, so widen it rather than shortening the table. The face is left blank on purpose; do not put a tiling layer on it. Defaults: at the top of the ring on the crest, Oval, 12 x 9 mm, 1.6 mm tall, top_flat 0.72, 1.4 mm shoulder."
    )]
    async fn add_signet_layer(
        &self,
        Parameters(p): Parameters<AddSignetParams>,
    ) -> Result<Json<LayerChange>, ErrorData> {
        let blend = p.blend.as_deref().map(parse_blend).transpose()?;
        let outline = p.outline.as_deref().map(parse_signet_outline).transpose()?;
        let mut applied = Vec::new();
        let mut s = SignetLayer::default();
        put_f64(&mut s.theta_deg, p.theta_deg, "theta_deg", &mut applied)?;
        put_f64(&mut s.v_mm, p.v_mm, "v_mm", &mut applied)?;
        if let Some(outline) = outline {
            s.outline = outline;
            applied.push(format!("outline={outline:?}"));
        }
        put_f64(&mut s.length_mm, p.length_mm, "length_mm", &mut applied)?;
        put_f64(&mut s.width_mm, p.width_mm, "width_mm", &mut applied)?;
        put_f64(&mut s.height_mm, p.height_mm, "height_mm", &mut applied)?;
        put_range(&mut s.top_flat, p.top_flat, "top_flat", 0.0, 1.0, &mut applied)?;
        put_f64(&mut s.shoulder_mm, p.shoulder_mm, "shoulder_mm", &mut applied)?;
        put_f64(&mut s.rotation_deg, p.rotation_deg, "rotation_deg", &mut applied)?;

        let name = p.name.unwrap_or_else(|| "Signet".to_string());
        let mut entry = LayerEntry::new(name, Layer::Signet(s));
        apply_entry_common(&mut entry, blend, p.opacity, &mut applied)?;
        let mut e = self.engine.lock();
        // Centre on the crest and size to the band unless the caller said
        // otherwise: a table wider than the band bows away from a true plane.
        if let Layer::Signet(s) = &mut entry.layer {
            let ctx = e.design().field_context();
            let fitted = SignetLayer::fitted_to(&ctx);
            if p.v_mm.is_none() {
                s.v_mm = fitted.v_mm;
                applied.push(format!("v_mm={:.3} (crest)", s.v_mm));
            }
            if p.width_mm.is_none() {
                s.width_mm = fitted.width_mm;
                applied.push(format!("width_mm={:.2} (fitted)", s.width_mm));
            }
            if p.length_mm.is_none() {
                s.length_mm = fitted.length_mm;
                applied.push(format!("length_mm={:.2} (fitted)", s.length_mm));
            }
            // Last, so it wins over both the caller's sizes and the fallbacks.
            if p.fill_head == Some(true) {
                s.fill_head(&ctx);
                applied.push(format!(
                    "fill_head: table {:.2} x {:.2} mm of {:.2} mm room across the head",
                    s.length_mm,
                    s.width_mm,
                    SignetLayer::room_across(&ctx)
                ));
            }
            if s.overhangs(&ctx) {
                applied.push(format!(
                    "warning: a {:.1} mm table on a {:.1} mm band overhangs the surface and will bow off flat",
                    s.width_mm, ctx.band_v_len_mm
                ));
            }
        }
        let change = push_layer(&mut e, entry, applied);
        drop(e);
        self.touch();
        Ok(Json(change))
    }

    #[tool(
        description = "Add a line of beads around the band. beads_around is an INTEGER count around the circumference, so the beads close on themselves with no seam and the pitch is the circumference divided by that count. bead_diameter_mm is their footprint, height_mm their peak, v_mm the position of the line in mm of v; `mirror` repeats the line the same distance in from the other edge. Castability: beads are hemispheres, so they hold draft anywhere except right on the crest line, where the sphere's own far side leans back under the pull. A typical milgrain is 0.45 mm beads 0.22 mm high on the flank just inside the edge, which casts cleanly in sand. Defaults: 120 beads at v = 0.8 mm, mirrored."
    )]
    async fn add_milgrain_layer(
        &self,
        Parameters(p): Parameters<AddMilgrainParams>,
    ) -> Result<Json<LayerChange>, ErrorData> {
        let blend = p.blend.as_deref().map(parse_blend).transpose()?;
        let mut applied = Vec::new();
        let mut m = MilgrainLayer::default();
        put_f64(&mut m.v_mm, p.v_mm, "v_mm", &mut applied)?;
        put_f64(&mut m.bead_diameter_mm, p.bead_diameter_mm, "bead_diameter_mm", &mut applied)?;
        put_u32(&mut m.beads_around, p.beads_around, "beads_around", &mut applied);
        put_f64(&mut m.height_mm, p.height_mm, "height_mm", &mut applied)?;
        put_bool(&mut m.mirror, p.mirror, "mirror", &mut applied);

        let name = p.name.unwrap_or_else(|| "Milgrain".to_string());
        let mut entry = LayerEntry::new(name, Layer::Milgrain(m));
        apply_entry_common(&mut entry, blend, p.opacity, &mut applied)?;
        let mut e = self.engine.lock();
        let change = push_layer(&mut e, entry, applied);
        drop(e);
        self.touch();
        Ok(Json(change))
    }

    #[tool(
        description = "Partially update one layer, found by its index in list_layers. name, enabled, blend, opacity and height_mm apply to any layer; every other field belongs to a specific kind (tiling, signet, border, gem seat pad, milgrain) and passing one the layer does not have is an error that names the layer's kind and lists the fields it does accept. Omitted fields keep their value. To change what a layer fundamentally is, remove it and add the kind you want. This is the tool that clears an undercut: height_mm in mm is the cheapest fix (halve it — the crest line takes only about 0.05 mm on a half-round band while the flanks take 0.3 to 0.5), and v_center_mm moves the whole layer off the crest toward a side face, where draft is perfect. Re-run castability after each change."
    )]
    async fn update_layer(
        &self,
        Parameters(p): Parameters<UpdateLayerParams>,
    ) -> Result<Json<LayerChange>, ErrorData> {
        let blend = p.blend.as_deref().map(parse_blend).transpose()?;
        let border_profile = p.profile.as_deref().map(parse_border_profile).transpose()?;
        let signet_outline = p.outline.as_deref().map(parse_signet_outline).transpose()?;
        let mut applied = Vec::new();
        let mut e = self.engine.lock();
        let len = e.design().layers.layers.len();
        if p.index >= len {
            drop(e);
            return Err(bad_index(p.index, len));
        }
        // Reject kind-specific fields before anything is written.
        {
            let entry = &e.design().layers.layers[p.index];
            let allowed = allowed_fields(&entry.layer);
            let rejected: Vec<&str> = kind_specific_present(&p)
                .into_iter()
                .filter(|f| !allowed.contains(f))
                .collect();
            if !rejected.is_empty() {
                let msg = format!(
                    "layer {} \"{}\" is a {} layer and has no field(s) {}; it accepts {} plus name, enabled, blend, opacity and height_mm",
                    p.index,
                    entry.name,
                    entry.layer.kind_label(),
                    rejected.join(", "),
                    allowed.join(", ")
                );
                drop(e);
                return Err(ErrorData::invalid_params(msg, None));
            }
        }

        let ctx = e.design().field_context();
        let d = e.design_mut();
        let entry = &mut d.layers.layers[p.index];
        if let Some(name) = p.name {
            applied.push(format!("name={name}"));
            entry.name = name;
        }
        put_bool(&mut entry.enabled, p.enabled, "enabled", &mut applied);
        if let Some(blend) = blend {
            entry.blend = blend;
            applied.push(format!("blend={blend:?}"));
        }
        put_range(&mut entry.opacity, p.opacity, "opacity", 0.0, 8.0, &mut applied)?;

        match &mut entry.layer {
            Layer::Openwork(o) => {
                if let Some(alpha) = p.alpha {
                    applied.push(format!("alpha={alpha}"));
                    o.tiling.alpha = alpha;
                }
                put_u32(&mut o.tiling.repeats_around, p.repeats_around, "repeats_around", &mut applied);
                put_f64(&mut o.tiling.v_center_mm, p.v_center_mm, "v_center_mm", &mut applied)?;
                put_f64(&mut o.tiling.v_span_mm, p.v_span_mm, "v_span_mm", &mut applied)?;
                put_f64(&mut o.keep_mm, p.keep_mm.or(p.height_mm), "keep_mm", &mut applied)?;
            }
            Layer::Tiling(t) => {
                if let Some(alpha) = p.alpha {
                    applied.push(format!("alpha={alpha}"));
                    t.alpha = alpha;
                }
                put_u32(&mut t.repeats_around, p.repeats_around, "repeats_around", &mut applied);
                put_u32(&mut t.rows, p.rows, "rows", &mut applied);
                put_f64(&mut t.v_center_mm, p.v_center_mm, "v_center_mm", &mut applied)?;
                put_f64(&mut t.v_span_mm, p.v_span_mm, "v_span_mm", &mut applied)?;
                put_f64(&mut t.rotation_deg, p.rotation_deg, "rotation_deg", &mut applied)?;
                put_f64(&mut t.offset_u, p.offset_u, "offset_u", &mut applied)?;
                put_f64(&mut t.offset_v, p.offset_v, "offset_v", &mut applied)?;
                put_f64(&mut t.height_mm, p.height_mm, "height_mm", &mut applied)?;
                put_f64(&mut t.gap_mm, p.gap_mm, "gap_mm", &mut applied)?;
                put_f64(&mut t.stagger, p.stagger, "stagger", &mut applied)?;
                put_bool(&mut t.mirror_alternate_u, p.mirror_alternate_u, "mirror_alternate_u", &mut applied);
                put_bool(&mut t.mirror_alternate_v, p.mirror_alternate_v, "mirror_alternate_v", &mut applied);
                put_range(&mut t.contrast, p.contrast, "contrast", 0.05, 8.0, &mut applied)?;
                put_range(&mut t.bias, p.bias, "bias", -1.0, 1.0, &mut applied)?;
                put_bool(&mut t.invert, p.invert, "invert", &mut applied);
                put_f64(&mut t.feather_mm, p.feather_mm, "feather_mm", &mut applied)?;
                put_bool(&mut t.continuous, p.continuous, "continuous", &mut applied);
                put_bool(&mut t.mirror_v, p.mirror_v, "mirror_v", &mut applied);
                if p.fit_to_side_faces == Some(true) {
                    fit_sides(t, &ctx, &mut applied)?;
                }
            }
            Layer::Border(b) => {
                put_f64(&mut b.v_mm, p.v_mm, "v_mm", &mut applied)?;
                put_f64(&mut b.width_mm, p.width_mm, "width_mm", &mut applied)?;
                put_f64(&mut b.height_mm, p.height_mm, "height_mm", &mut applied)?;
                if let Some(profile) = border_profile {
                    b.profile = profile;
                    applied.push(format!("profile={profile:?}"));
                }
                put_bool(&mut b.mirror, p.mirror, "mirror", &mut applied);
                put_u32(&mut b.rope_twists, p.rope_twists, "rope_twists", &mut applied);
            }
            Layer::SeatPad(s) => {
                put_f64(&mut s.theta_deg, p.theta_deg, "theta_deg", &mut applied)?;
                put_f64(&mut s.v_mm, p.v_mm, "v_mm", &mut applied)?;
                put_f64(&mut s.diameter_mm, p.diameter_mm, "diameter_mm", &mut applied)?;
                put_range(&mut s.elong, p.elong, "elong", 1.0, 8.0, &mut applied)?;
                put_f64(&mut s.rot_deg, p.rot_deg, "rot_deg", &mut applied)?;
                put_f64(&mut s.height_mm, p.height_mm, "height_mm", &mut applied)?;
                put_range(&mut s.crown, p.crown, "crown", 0.0, 1.0, &mut applied)?;
                put_f64(&mut s.blend_mm, p.blend_mm, "blend_mm", &mut applied)?;
            }
            Layer::Signet(s) => {
                put_f64(&mut s.theta_deg, p.theta_deg, "theta_deg", &mut applied)?;
                put_f64(&mut s.v_mm, p.v_mm, "v_mm", &mut applied)?;
                if let Some(outline) = signet_outline {
                    s.outline = outline;
                    applied.push(format!("outline={outline:?}"));
                }
                put_f64(&mut s.length_mm, p.length_mm, "length_mm", &mut applied)?;
                put_f64(&mut s.width_mm, p.width_mm, "width_mm", &mut applied)?;
                put_f64(&mut s.height_mm, p.height_mm, "height_mm", &mut applied)?;
                put_range(&mut s.top_flat, p.top_flat, "top_flat", 0.0, 1.0, &mut applied)?;
                put_f64(&mut s.shoulder_mm, p.shoulder_mm, "shoulder_mm", &mut applied)?;
                put_f64(&mut s.rotation_deg, p.rotation_deg, "rotation_deg", &mut applied)?;
            }
            Layer::Milgrain(m) => {
                put_f64(&mut m.v_mm, p.v_mm, "v_mm", &mut applied)?;
                put_f64(&mut m.bead_diameter_mm, p.bead_diameter_mm, "bead_diameter_mm", &mut applied)?;
                put_u32(&mut m.beads_around, p.beads_around, "beads_around", &mut applied);
                put_f64(&mut m.height_mm, p.height_mm, "height_mm", &mut applied)?;
                put_bool(&mut m.mirror, p.mirror, "mirror", &mut applied);
            }
            // A group has no per-kind fields; the common entry fields above
            // (name, enabled, blend, opacity) already applied.
            Layer::Group(_) => {}
            Layer::Curve(l) => {
                put_u32(&mut l.repeats_around, p.repeats_around, "repeats_around", &mut applied);
                put_f64(&mut l.width_mm, p.width_mm, "width_mm", &mut applied)?;
                put_f64(&mut l.height_mm, p.height_mm, "height_mm", &mut applied)?;
            }
            Layer::Flutes(l) => {
                put_u32(&mut l.count, p.repeats_around, "repeats_around", &mut applied);
                put_f64(&mut l.width_mm, p.width_mm, "width_mm", &mut applied)?;
                put_f64(&mut l.height_mm, p.height_mm, "height_mm", &mut applied)?;
            }
            Layer::Decals(l) => {
                if let Some(alpha) = p.alpha {
                    applied.push(format!("alpha={alpha}"));
                    l.alpha = alpha;
                }
                if let Some(h) = p.height_mm {
                    for s in &mut l.decals {
                        s.height_mm = h;
                    }
                    applied.push(format!("height_mm={h}"));
                }
            }
            Layer::SeatRun(l) => {
                put_u32(&mut l.count, p.repeats_around, "repeats_around", &mut applied);
                put_f64(&mut l.seat.v_mm, p.v_mm, "v_mm", &mut applied)?;
                put_f64(&mut l.seat.height_mm, p.height_mm, "height_mm", &mut applied)?;
            }
        }
        let change = LayerChange {
            generation: e.generation(),
            index: Some(p.index),
            applied,
            layers: layer_infos(e.design()),
        };
        drop(e);
        self.touch();
        Ok(Json(change))
    }

    #[tool(
        description = "Remove the layer at this index. Indices above it shift down by one, so remove from the highest index first when dropping several. Use list_layers to confirm what is there."
    )]
    async fn remove_layer(
        &self,
        Parameters(p): Parameters<LayerIndexParams>,
    ) -> Result<Json<LayerChange>, ErrorData> {
        let mut e = self.engine.lock();
        let len = e.design().layers.layers.len();
        if p.index >= len {
            drop(e);
            return Err(bad_index(p.index, len));
        }
        let removed = e.design_mut().layers.layers.remove(p.index);
        let change = LayerChange {
            generation: e.generation(),
            index: None,
            applied: vec![format!("removed={}", removed.name)],
            layers: layer_infos(e.design()),
        };
        drop(e);
        self.touch();
        Ok(Json(change))
    }

    #[tool(
        description = "Move a layer through the stack by `delta` places; negative moves it earlier (composited sooner), positive later, and the target is clamped to the ends of the stack. Order matters because blends composite in order: a Replace layer discards everything beneath it, and a Subtract layer only carves what is already there. Returns the new index."
    )]
    async fn move_layer(
        &self,
        Parameters(p): Parameters<MoveLayerParams>,
    ) -> Result<Json<LayerChange>, ErrorData> {
        let mut e = self.engine.lock();
        let len = e.design().layers.layers.len();
        if p.index >= len {
            drop(e);
            return Err(bad_index(p.index, len));
        }
        let target = (p.index as i64 + p.delta as i64).clamp(0, len as i64 - 1) as usize;
        if target != p.index {
            let d = e.design_mut();
            let entry = d.layers.layers.remove(p.index);
            d.layers.layers.insert(target, entry);
        }
        let change = LayerChange {
            generation: e.generation(),
            index: Some(target),
            applied: vec![format!("moved {} -> {target}", p.index)],
            layers: layer_infos(e.design()),
        };
        drop(e);
        self.touch();
        Ok(Json(change))
    }

    #[tool(
        description = "Enable or disable one layer without removing it. A disabled layer contributes nothing to the height field but keeps its settings, which is the cheap way to isolate what a single layer costs in the castability report."
    )]
    async fn set_layer_enabled(
        &self,
        Parameters(p): Parameters<SetLayerEnabledParams>,
    ) -> Result<Json<LayerChange>, ErrorData> {
        let mut e = self.engine.lock();
        let len = e.design().layers.layers.len();
        if p.index >= len {
            drop(e);
            return Err(bad_index(p.index, len));
        }
        e.design_mut().layers.layers[p.index].enabled = p.enabled;
        let change = LayerChange {
            generation: e.generation(),
            index: Some(p.index),
            applied: vec![format!("enabled={}", p.enabled)],
            layers: layer_infos(e.design()),
        };
        drop(e);
        self.touch();
        Ok(Json(change))
    }

    #[tool(
        description = "Limit one layer to an arc of the ring instead of the whole circumference, or keep it everywhere except that arc. Works on any layer kind. theta_deg is the centre (90 is the top of the ring), span_deg the arc held at full strength, fade_deg the falloff at each end, and invert flips it to everything-but-the-arc. This is what puts ornament on the shoulders flanking a signet head without running it across the table: window the ornament with invert=true over the head's angular span. Leave some fade — a hard end raises a wall the mould has to clear. Pass enabled=false to go back to running the whole way round."
    )]
    async fn set_layer_window(
        &self,
        Parameters(p): Parameters<SetLayerWindowParams>,
    ) -> Result<Json<LayerChange>, ErrorData> {
        let mut applied = Vec::new();
        let mut e = self.engine.lock();
        let len = e.design().layers.layers.len();
        if p.index >= len {
            drop(e);
            return Err(bad_index(p.index, len));
        }
        let w = &mut e.design_mut().layers.layers[p.index].window;
        // Setting any shape field turns the window on unless told otherwise.
        let shaped = p.theta_deg.is_some() || p.span_deg.is_some() || p.fade_deg.is_some();
        put_bool(&mut w.enabled, p.enabled.or(shaped.then_some(true)), "enabled", &mut applied);
        put_f64(&mut w.theta_deg, p.theta_deg, "theta_deg", &mut applied)?;
        put_range(&mut w.span_deg, p.span_deg, "span_deg", 0.0, 360.0, &mut applied)?;
        put_range(&mut w.fade_deg, p.fade_deg, "fade_deg", 0.0, 180.0, &mut applied)?;
        put_bool(&mut w.invert, p.invert, "invert", &mut applied);
        if w.enabled && w.fade_deg < 1.0 && w.span_deg > 0.0 {
            applied.push("warning: no fade leaves a vertical wall at each end of the arc".into());
        }
        let change = LayerChange {
            generation: e.generation(),
            index: Some(p.index),
            applied,
            layers: layer_infos(e.design()),
        };
        drop(e);
        self.touch();
        Ok(Json(change))
    }

    #[tool(
        description = "Remove every layer, leaving the bare band: the swept profile with no height field at all. That surface is undercut-free by construction, so this is the way back to a known-castable starting point."
    )]
    async fn clear_layers(&self) -> Json<LayerChange> {
        let mut e = self.engine.lock();
        let removed = e.design().layers.layers.len();
        e.design_mut().layers.layers.clear();
        let change = LayerChange {
            generation: e.generation(),
            index: None,
            applied: vec![format!("cleared={removed}")],
            layers: Vec::new(),
        };
        drop(e);
        self.touch();
        Json(change)
    }

    #[tool(
        description = "Rebuild the mesh and return the report: watertight check with boundary and non-manifold edge counts, triangle and vertex counts, volume in mm3, surface area, overall size, inner and outer diameter, band width, the tallest and deepest displacement the layer stack applied, cast weight in ten metals, and the build time. theta_steps and profile_steps override the design's resolution for this build only and are not stored. The mesh is a torus grid closed in both directions, so it is watertight by construction — a failed watertight check means a degenerate parameter, not a hole to patch."
    )]
    async fn build(
        &self,
        Parameters(p): Parameters<BuildParamsOverride>,
    ) -> Json<ReportJson> {
        let mut e = self.engine.lock();
        let mut params = e.design().build;
        if let Some(v) = p.theta_steps {
            params.theta_steps = (v as usize).clamp(24, 4096);
        }
        if let Some(v) = p.profile_steps {
            params.profile_steps = (v as usize).clamp(24, 1024);
        }
        let report = e.build(Some(params));
        let generation = e.generation();
        drop(e);
        self.touch();
        Json(report_json(&report, params, generation))
    }

    #[tool(
        description = "Analyse the current mesh for a two-part sand mould that parts perpendicular to the finger axis and pulls in both directions, building first if the design changed. Returns the verdict (Castable, Marginal, or NotCastable), per-class face counts with their areas in mm2 (good draft, marginal, vertical wall, undercut), the undercut share of the total surface, the worst draft angle found in degrees (negative means a face leans back under itself and will lock in the sand), the parting height in mm, and the notes. Read the notes: they are plain language, returned verbatim, and they say what to cut and where to move it. A face is called marginal below the design's min_draft_deg, 3 degrees by default for Delft clay or petrobond. The finger hole is always reported as a vertical wall rather than an undercut, because it cores in the sand or is reamed at the bench. When the design carries seat pads or seat runs, `stones` holds the bench checks: per seat, what the base surface under it is (a side face is castable by construction; a crown reports its draft), the metal from its foot to the band edge, the metal available for the stone's pavilion before the 0.5 mm minimum wall, the bridge between neighbouring stones in a run, and any warnings — plus the stone count and total carats. Stones themselves are never cast; the checks are about the stock the ring casts for the bench to set into. `field` is the authoritative verdict: it samples the true surface with smooth normals instead of reading mesh facets, so the crest-line and signet-table phantoms a refined mesh reports cannot appear in it, and it carries the thinnest outer-to-bore wall over the finger hole against min_section_mm — trust `field.verdict` when it and the mesh numbers disagree."
    )]
    async fn castability(&self) -> Json<CastJson> {
        let mut e = self.engine.lock();
        let cast = e.castability();
        let min_draft = e.design().draft.min_draft_deg;
        let generation = e.generation();
        let stones =
            ringdesign_core::stones::report(e.design(), cast.parting_z_mm).map(|r| stones_json(&r));
        let field = field_json(&e.field_report());
        let dfm = ringdesign_core::dfm::findings(e.design())
            .into_iter()
            .map(|f| format!("{}: {}", f.label, f.message))
            .collect();
        drop(e);
        self.touch();
        Json(cast_json(&cast, min_draft, generation, stones, field, dfm))
    }

    #[tool(
        description = "Slice the displaced ring at one angle (theta_deg, 90 is the top of the ring) and report the cross-section, all lengths in mm: radial and axial extents, the parting height, min_wall_mm — the thinnest metal between the outer surface and the bore, which is the number to check against the 0.5 mm a sand pour needs to fill — how many segments undercut, and the points themselves, each carrying radius, height, signed draft angle in degrees, and class. The point list is DOWNSAMPLED to about 120 points out of `steps` sampled; `sampled_points` and `returned_points` say by how much. It evaluates the same profile and height field as the mesh build, so the section never disagrees with the solid. Use it to see where in v a layer undercuts rather than only how much: points with class \"Undercut\" and surface true are height-field damage, while a \"Vertical wall\" at small radius is just the bore."
    )]
    async fn cross_section(&self, Parameters(p): Parameters<SectionParams>) -> Json<SectionJson> {
        let e = self.engine.lock();
        let theta = p.theta_deg.filter(|t| t.is_finite()).unwrap_or(90.0);
        let steps = p
            .steps
            .map(|s| s as usize)
            .unwrap_or(e.design().build.profile_steps)
            .clamp(24, 4096);
        let section = e.section(theta, steps);
        drop(e);
        Json(section_json(&section))
    }

    #[tool(
        description = "Write the current mesh as a binary STL, building first if the design changed. `path` is optional and defaults to a temp file named after the design. Returns the path and the byte count. The export uses the design's stored resolution, so call set_build_params first if you have been iterating at draft quality — 1024 x 320 is the usual export setting."
    )]
    async fn export_stl(
        &self,
        Parameters(p): Parameters<ExportParams>,
    ) -> Result<Json<ExportResult>, ErrorData> {
        let mut e = self.engine.lock();
        let path = p.path.unwrap_or_else(|| default_export_path(&e.design().name, "stl"));
        let bytes = e.export_stl(&path);
        drop(e);
        self.touch();
        let bytes =
            bytes.map_err(|err| ErrorData::internal_error(format!("write {path}: {err}"), None))?;
        Ok(Json(ExportResult { path, bytes }))
    }

    #[tool(
        description = "Write the current mesh as a Wavefront OBJ with smooth vertex normals, building first if the design changed. `path` is optional and defaults to a temp file named after the design. Returns the path and the byte count. OBJ carries the design name as the object name; use STL for the caster and OBJ for anything that wants the shading normals."
    )]
    async fn export_obj(
        &self,
        Parameters(p): Parameters<ExportParams>,
    ) -> Result<Json<ExportResult>, ErrorData> {
        let mut e = self.engine.lock();
        let path = p.path.unwrap_or_else(|| default_export_path(&e.design().name, "obj"));
        let bytes = e.export_obj(&path);
        drop(e);
        self.touch();
        let bytes =
            bytes.map_err(|err| ErrorData::internal_error(format!("write {path}: {err}"), None))?;
        Ok(Json(ExportResult { path, bytes }))
    }

    #[tool(
        description = "Write the current mesh as a 3MF package, building first if the design changed. `path` is optional and defaults to a temp file named after the design. Returns the path and the byte count. 3MF is a zip with the model as XML that states unit=millimeter and carries the design name and ring size as metadata, so a slicer or CAD package opens it at the right scale without being told — use it over STL wherever the receiver understands it, because STL has no units at all."
    )]
    async fn export_3mf(
        &self,
        Parameters(p): Parameters<ExportParams>,
    ) -> Result<Json<ExportResult>, ErrorData> {
        let mut e = self.engine.lock();
        let path = p.path.unwrap_or_else(|| default_export_path(&e.design().name, "3mf"));
        let bytes = e.export_3mf(&path);
        drop(e);
        self.touch();
        let bytes =
            bytes.map_err(|err| ErrorData::internal_error(format!("write {path}: {err}"), None))?;
        Ok(Json(ExportResult { path, bytes }))
    }

    #[tool(
        description = "Write the current mesh as a glTF binary (.glb), building first if the design changed. `path` is optional and defaults to a temp file named after the design. One node, smooth normals, a PBR metal material; coordinates are scaled from millimetres to metres because glTF's units are metres — every viewer then shows a ring-sized ring. For casting use STL or 3MF; this one is for renders, web viewers and scene tools."
    )]
    async fn export_glb(
        &self,
        Parameters(p): Parameters<ExportParams>,
    ) -> Result<Json<ExportResult>, ErrorData> {
        let mut e = self.engine.lock();
        let path = p.path.unwrap_or_else(|| default_export_path(&e.design().name, "glb"));
        let bytes = e.export_glb(&path);
        drop(e);
        self.touch();
        let bytes =
            bytes.map_err(|err| ErrorData::internal_error(format!("write {path}: {err}"), None))?;
        Ok(Json(ExportResult { path, bytes }))
    }

    #[tool(
        description = "Save the design to `path` as JSON: size, profile, shank, the whole layer stack, build resolution and casting settings. Alphas are referenced by name, not embedded, so a design file is small but needs the same library to rebuild identically. The conventional extension is .ring.json."
    )]
    async fn save_design(
        &self,
        Parameters(p): Parameters<PathParams>,
    ) -> Result<Json<FileResult>, ErrorData> {
        let e = self.engine.lock();
        let result = e.save_design(&p.path);
        let generation = e.generation();
        let summary = one_line(e.design());
        drop(e);
        result.map_err(|err| ErrorData::internal_error(format!("save {}: {err}", p.path), None))?;
        Ok(Json(FileResult { path: p.path, generation, summary }))
    }

    #[tool(
        description = "Load a design from a JSON file, replacing everything currently in the engine — design, layers, build and casting settings. Any layer referencing an alpha that is not in this library contributes nothing to the height field, so check list_alphas against the layer list afterwards if the ring comes back plain."
    )]
    async fn load_design(
        &self,
        Parameters(p): Parameters<PathParams>,
    ) -> Result<Json<FileResult>, ErrorData> {
        let mut e = self.engine.lock();
        let result = e.load_design(&p.path);
        let generation = e.generation();
        let summary = one_line(e.design());
        drop(e);
        self.touch();
        result.map_err(|err| ErrorData::internal_error(format!("load {}: {err}", p.path), None))?;
        Ok(Json(FileResult { path: p.path, generation, summary }))
    }

    #[tool(
        description = "Start a fresh design from a curated template, replacing everything in the engine. Call with no name to list the templates with their blurbs. Every template references only builtin alphas and every part of it is an ordinary editable layer or shank setting afterwards."
    )]
    async fn apply_template(
        &self,
        Parameters(p): Parameters<TemplateParams>,
    ) -> Result<Json<DesignChange>, ErrorData> {
        let templates = ringdesign_core::templates::all();
        let Some(name) = p.name else {
            let list: Vec<String> =
                templates.iter().map(|t| format!("{} — {}", t.name, t.blurb)).collect();
            return Ok(Json(DesignChange {
                generation: 0,
                applied: list,
                summary: "no template applied; call again with one of these names".into(),
            }));
        };
        let Some(t) = templates.iter().find(|t| t.name.eq_ignore_ascii_case(&name)) else {
            let names: Vec<&str> = templates.iter().map(|t| t.name).collect();
            return Err(ErrorData::invalid_params(
                format!("no template named {name:?}; the templates are {names:?}"),
                None,
            ));
        };
        let mut e = self.engine.lock();
        *e.design_mut() = t.design();
        let change = DesignChange {
            generation: e.generation(),
            applied: vec![format!("template={}", t.name)],
            summary: one_line(e.design()),
        };
        drop(e);
        self.touch();
        Ok(Json(change))
    }
}

/// Snap a tiling onto the faces square to the mould pull.
fn fit_sides(
    t: &mut TilingLayer,
    ctx: &ringdesign_core::FieldContext,
    applied: &mut Vec<String>,
) -> Result<(), ErrorData> {
    if !t.fit_to_side_faces(ctx, SIDE_FACE_MIN_DRAFT_DEG) {
        return Err(ErrorData::invalid_params(
            format!(
                "this profile has no face within {SIDE_FACE_MIN_DRAFT_DEG:.0} degrees of square                  to the mould pull, so there is no side face to fit to. Square the sides with                  set_profile side_draft_deg=0 and a small edge_round_mm, choose a flatter style,                  or add an edge flange."
            ),
            None,
        ));
    }
    applied.push(format!(
        "fit_to_side_faces: v {:.2}..{:.2} mm, {} tiles, mirror_v={}",
        t.v_bounds().0,
        t.v_bounds().1,
        t.repeats_around,
        t.mirror_v
    ));
    Ok(())
}

/// Apply the blend and opacity every layer entry carries.
fn apply_entry_common(
    entry: &mut LayerEntry,
    blend: Option<Blend>,
    opacity: Option<f64>,
    applied: &mut Vec<String>,
) -> Result<(), ErrorData> {
    if let Some(blend) = blend {
        entry.blend = blend;
        applied.push(format!("blend={blend:?}"));
    }
    put_range(&mut entry.opacity, opacity, "opacity", 0.0, 8.0, applied)
}

/// Append a layer and describe the resulting stack.
fn push_layer(
    engine: &mut ringdesign_core::DesignEngine,
    entry: LayerEntry,
    applied: Vec<String>,
) -> LayerChange {
    let d = engine.design_mut();
    d.layers.layers.push(entry);
    let index = d.layers.layers.len() - 1;
    LayerChange {
        generation: engine.generation(),
        index: Some(index),
        applied,
        layers: layer_infos(engine.design()),
    }
}

#[tool_handler]
impl ServerHandler for RingDesignServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        // Without this the handshake reports rmcp's own crate name and version.
        .with_server_info(
            Implementation::new("ringdesign", env!("CARGO_PKG_VERSION"))
                .with_title("RingDesigner")
                .with_description(
                    "Procedural generator for ring designs cast in sand: sweeps a domed \
                     cross-section, lays decorative relief on it as a height field, and reports \
                     whether the result pulls from a two-part mould.",
                ),
        )
        .with_instructions(
            "RingDesigner — procedural generator for ring designs that must be cast in SAND \
             (Delft clay / petrobond), not lost wax.\n\n\
             Model: a ring is one closed cross-section swept 360 degrees about the finger axis \
             (Z). Everything decorative is a scalar height field h(u, v) in mm displacing that \
             surface along its normal. u is arc distance around the ring and wraps at the \
             circumference; v is arc distance across the cross-section, from one bore edge over \
             the outer surface to the other. Tiling, borders, milgrain and gem seat pads are all \
             layers in that one field — there is no CSG.\n\n\
             Casting constraint: the mould parts on a plane perpendicular to Z and pulls in both \
             directions. The two side faces have perfect draft, so relief there always pulls. The \
             outer surface releases only where it domes away from a single crest, and the crest \
             line itself is tangent to the parting plane — relief there undercuts almost at once \
             (about 0.05 mm on a half-round band), while the same texture on the flanks is fine. \
             The bore is a straight through-hole and is never an undercut.\n\n\
             Seamlessness: anything positioned by an integer count around the ring closes on \
             itself by construction — tiling repeats_around, milgrain beads_around, border \
             rope_twists.\n\n\
             Workflow: describe_ring to see where you are, list_profile_styles / list_shank_styles \
             / list_alphas to see what is available, set_ring / set_profile / set_shank to shape \
             the band, add_* to lay on pattern, build for the report and castability for the \
             verdict and notes, cross_section to find where a layer undercuts, export_stl when it \
             passes. Every mutation bumps a generation counter that a GUI sharing this engine \
             polls, so edits appear on both sides.",
        )
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListResourcesResult, ErrorData> {
        Ok(crate::resources::list())
    }

    async fn list_resource_templates(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListResourceTemplatesResult, ErrorData> {
        Ok(crate::resources::templates())
    }

    async fn read_resource(
        &self,
        request: rmcp::model::ReadResourceRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ReadResourceResponse, ErrorData> {
        self.read_ring_uri(&request.uri).map(Into::into)
    }

    async fn list_prompts(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListPromptsResult, ErrorData> {
        Ok(crate::prompts::list())
    }

    async fn get_prompt(
        &self,
        request: rmcp::model::GetPromptRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::GetPromptResponse, ErrorData> {
        crate::prompts::get(&request.name, request.arguments.as_ref()).map(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::alpha::Alpha;
    use ringdesign_core::{DesignEngine, SharedEngine};

    fn tiny_lib() -> AlphaLibrary {
        let mut lib = AlphaLibrary::default();
        for name in ["Rope", "Braid", "Basketweave", "Chevron", "Bark rough", "bark fine"] {
            lib.insert(Alpha::new(name, 2, 2, vec![0.0, 1.0, 1.0, 0.0]));
        }
        lib
    }

    fn engine() -> SharedEngine {
        DesignEngine::shared(tiny_lib())
    }

    fn server() -> RingDesignServer {
        RingDesignServer::new(engine())
    }

    /// `Json` is not Debug, so `unwrap_err` does not apply.
    fn expect_err<T>(r: Result<T, ErrorData>) -> ErrorData {
        match r {
            Ok(_) => panic!("expected an error"),
            Err(e) => e,
        }
    }

    async fn add_tiling(s: &RingDesignServer, alpha: &str) -> LayerChange {
        s.add_tiling_layer(Parameters(AddTilingParams {
            alpha: alpha.into(),
            name: None,
            blend: None,
            opacity: None,
            repeats_around: None,
            rows: None,
            v_center_mm: None,
            v_span_mm: None,
            rotation_deg: None,
            offset_u: None,
            offset_v: None,
            height_mm: None,
            gap_mm: None,
            stagger: None,
            mirror_alternate_u: None,
            mirror_alternate_v: None,
            contrast: None,
            bias: None,
            invert: None,
            feather_mm: None,
            continuous: None,
            mirror_v: None,
            fit_to_side_faces: None,
        }))
        .await
        .expect("alpha is in the library")
        .0
    }

    /// Square the profile's sides so it actually has faces to fit to.
    async fn square_the_sides(s: &RingDesignServer) {
        let mut e = s.engine.lock();
        let d = e.design_mut();
        d.profile.apply_style(ringdesign_core::ProfileStyle::Flat);
        d.profile.flatten_sides();
    }

    #[tokio::test]
    async fn fitting_to_side_faces_needs_a_profile_that_has_some() {
        let s = server();
        // The default half round is all dome, so the fit must refuse and say why.
        add_tiling(&s, "Rope").await;
        let err = expect_err(
            s.update_layer(Parameters(UpdateLayerParams {
                index: 0,
                fit_to_side_faces: Some(true),
                ..Default::default()
            }))
            .await,
        );
        let msg = format!("{err:?}");
        assert!(msg.contains("side face"), "unhelpful error: {msg}");
        assert!(msg.contains("side_draft_deg"), "the error does not say how to fix it: {msg}");

        square_the_sides(&s).await;
        let out = s
            .update_layer(Parameters(UpdateLayerParams {
                index: 0,
                fit_to_side_faces: Some(true),
                ..Default::default()
            }))
            .await
            .expect("squared sides should fit")
            .0;
        assert!(
            out.applied.iter().any(|a| a.contains("fit_to_side_faces")),
            "the fit was not reported: {:?}",
            out.applied
        );
        let e = s.engine.lock();
        match &e.design().layers.layers[0].layer {
            Layer::Tiling(t) => {
                assert!(t.mirror_v, "a symmetric profile should be decorated both sides");
                let (v0, v1) = t.v_bounds();
                assert!(v1 - v0 > 0.3, "fitted strip is only {:.2} mm", v1 - v0);
            }
            other => panic!("expected a tiling layer, got {}", other.kind_label()),
        }
    }

    #[tokio::test]
    async fn describe_ring_reports_side_faces_only_when_the_profile_has_them() {
        let s = server();
        assert!(s.describe_ring().await.0.side_faces.is_none(), "a dome has no side face");
        square_the_sides(&s).await;
        let f = s.describe_ring().await.0.side_faces.expect("squared sides expose faces");
        assert!(f.even, "a symmetric profile should report even faces");
        let low = f.low_mm.expect("a squared profile has a low face");
        assert!(low[1] > low[0]);
    }

    #[tokio::test]
    async fn a_window_gates_a_layer_and_reads_back_in_the_listing() {
        let s = server();
        add_tiling(&s, "Rope").await;
        let out = s
            .set_layer_window(Parameters(SetLayerWindowParams {
                index: 0,
                theta_deg: Some(90.0),
                span_deg: Some(80.0),
                fade_deg: Some(0.0),
                invert: Some(true),
                ..Default::default()
            }))
            .await
            .unwrap()
            .0;
        assert!(
            out.applied.iter().any(|a| a.contains("warning")),
            "a hard-edged window should warn: {:?}",
            out.applied
        );
        let listed = s.list_layers().await.0;
        let w = listed.layers[0].window.as_deref().expect("window should be listed");
        assert!(w.contains("everywhere but"), "inverted window read back as {w}");

        // The gate must actually reach the field.
        let e = s.engine.lock();
        let d = e.design();
        let ctx = d.field_context();
        let on_head = d.layers.height(
            ringdesign_core::Uv { u: ctx.u_of_theta(90.0), v: ctx.crest_v_mm },
            &ctx,
            e.library(),
        );
        let off_head = d.layers.height(
            ringdesign_core::Uv { u: ctx.u_of_theta(270.0), v: ctx.crest_v_mm },
            &ctx,
            e.library(),
        );
        assert_eq!(on_head, 0.0, "the inverted window did not clear the head");
        assert!(off_head > 0.0, "the layer vanished everywhere");
    }

    #[tokio::test]
    async fn disabling_a_window_puts_the_layer_back_round_the_whole_ring() {
        let s = server();
        add_tiling(&s, "Rope").await;
        s.set_layer_window(Parameters(SetLayerWindowParams {
            index: 0,
            span_deg: Some(40.0),
            ..Default::default()
        }))
        .await
        .unwrap();
        assert!(s.engine.lock().design().layers.layers[0].window.enabled);
        s.set_layer_window(Parameters(SetLayerWindowParams {
            index: 0,
            enabled: Some(false),
            ..Default::default()
        }))
        .await
        .unwrap();
        assert!(!s.engine.lock().design().layers.layers[0].window.enabled);
        assert!(s.list_layers().await.0.layers[0].window.is_none());
    }

    #[tokio::test]
    async fn a_partial_set_profile_leaves_the_other_fields_alone() {
        let s = server();
        let before = s.engine.lock().design().profile;
        let out = s
            .set_profile(Parameters(SetProfileParams {
                width_mm: Some(4.5),
                ..Default::default()
            }))
            .await
            .unwrap()
            .0;
        assert_eq!(out.applied, vec!["width_mm=4.5".to_string()]);

        let after = s.engine.lock().design().profile;
        assert_eq!(after.width_mm, 4.5);
        assert_eq!(after.thickness_mm, before.thickness_mm);
        assert_eq!(after.crown_mm, before.crown_mm);
        assert_eq!(after.shape_a, before.shape_a);
        assert_eq!(after.shape_b, before.shape_b);
        assert_eq!(after.edge_round_mm, before.edge_round_mm);
        assert_eq!(after.comfort_fit_mm, before.comfort_fit_mm);
        assert_eq!(after.side_draft_deg, before.side_draft_deg);
        assert_eq!(after.style, before.style);
    }

    #[tokio::test]
    async fn set_profile_applies_the_style_before_the_overrides() {
        let s = server();
        let out = s
            .set_profile(Parameters(SetProfileParams {
                style: Some("knife edge".into()),
                crown_mm: Some(0.4),
                ..Default::default()
            }))
            .await
            .unwrap()
            .0;
        let p = s.engine.lock().design().profile;
        assert_eq!(p.style, ProfileStyle::KnifeEdge);
        // The preset would have set crown to the full thickness.
        assert_eq!(p.crown_mm, 0.4, "the override did not win: {:?}", out.applied);
        assert_eq!(p.shape_a, 1.0);
        assert_eq!(p.shape_b, 1.0);

        // Thickness in the same call re-derives what the caller did not pin.
        s.set_profile(Parameters(SetProfileParams {
            style: Some("HalfRound".into()),
            thickness_mm: Some(4.0),
            ..Default::default()
        }))
        .await
        .unwrap();
        let p = s.engine.lock().design().profile;
        assert_eq!(p.thickness_mm, 4.0);
        assert!((p.crown_mm - 3.6).abs() < 1e-9, "crown {} is not 0.9 of 4 mm", p.crown_mm);
    }

    #[tokio::test]
    async fn set_profile_rejects_an_unknown_style_and_names_the_valid_ones() {
        let s = server();
        let err = s
            .set_profile(Parameters(SetProfileParams {
                style: Some("domed".into()),
                ..Default::default()
            }))
            .await;
        let err = expect_err(err);
        let msg = err.message.to_string();
        assert!(msg.contains("domed"), "{msg}");
        assert!(msg.contains("HalfRound") && msg.contains("KnifeEdge"), "{msg}");
        // Nothing was written.
        assert_eq!(s.engine.lock().design().profile.style, ProfileStyle::HalfRound);
    }

    #[tokio::test]
    async fn update_layer_on_a_mismatched_kind_errors_instead_of_no_op() {
        let s = server();
        s.add_milgrain_layer(Parameters(AddMilgrainParams::default())).await.unwrap();
        let err = s
            .update_layer(Parameters(UpdateLayerParams {
                index: 0,
                alpha: Some("Rope".into()),
                repeats_around: Some(12),
                ..blank_update(0)
            }))
            .await;
        let err = expect_err(err);
        let msg = err.message.to_string();
        assert!(msg.contains("Milgrain"), "{msg}");
        assert!(msg.contains("alpha") && msg.contains("repeats_around"), "{msg}");
        assert!(msg.contains("beads_around"), "the accepted fields are missing: {msg}");

        // A field the kind does own still works, and nothing was half-applied.
        let out = s
            .update_layer(Parameters(UpdateLayerParams {
                index: 0,
                beads_around: Some(96),
                ..blank_update(0)
            }))
            .await
            .unwrap()
            .0;
        assert_eq!(out.applied, vec!["beads_around=96".to_string()]);
    }

    #[tokio::test]
    async fn update_layer_and_remove_layer_reject_an_out_of_range_index() {
        let s = server();
        let err = s
            .remove_layer(Parameters(LayerIndexParams { index: 0 }))
            .await;
        let err = expect_err(err);
        assert!(err.message.contains("empty"), "{}", err.message);

        add_tiling(&s, "Rope").await;
        let err = s
            .remove_layer(Parameters(LayerIndexParams { index: 4 }))
            .await;
        let err = expect_err(err);
        assert!(err.message.contains("0..0"), "{}", err.message);
        let err = expect_err(s.update_layer(Parameters(blank_update(9))).await);
        assert!(err.message.contains("out of range"), "{}", err.message);
        assert_eq!(s.engine.lock().design().layers.layers.len(), 1);
    }

    #[tokio::test]
    async fn every_mutation_advances_the_generation() {
        let s = server();
        let g0 = s.engine.lock().generation();
        let a = s
            .set_ring(Parameters(SetRingParams { name: Some("Vine".into()), size: Some(8.13) }))
            .await
            .unwrap()
            .0;
        assert!(a.generation > g0);
        assert_eq!(s.engine.lock().design().size.0, 8.25, "size was not quartered");

        let b = add_tiling(&s, "Braid").await;
        assert!(b.generation > a.generation);
        let c = s.clear_layers().await.0;
        assert!(c.generation > b.generation);
        assert!(c.layers.is_empty());
    }

    #[tokio::test]
    async fn layers_move_and_toggle_by_index() {
        let s = server();
        add_tiling(&s, "Rope").await;
        add_tiling(&s, "Braid").await;
        let out = s
            .move_layer(Parameters(MoveLayerParams { index: 1, delta: -5 }))
            .await
            .unwrap()
            .0;
        assert_eq!(out.index, Some(0));
        assert_eq!(out.layers[0].name, "Braid");

        let out = s
            .set_layer_enabled(Parameters(SetLayerEnabledParams { index: 1, enabled: false }))
            .await
            .unwrap()
            .0;
        assert!(!out.layers[1].enabled);
    }

    #[tokio::test]
    async fn list_alphas_honours_the_filter_and_the_limit() {
        let s = server();
        let all = s.list_alphas(Parameters(ListAlphasParams::default())).await.0;
        assert_eq!(all.total, 6);
        assert_eq!(all.matched, 6);
        assert_eq!(all.returned, 6);

        let capped = s
            .list_alphas(Parameters(ListAlphasParams { filter: None, limit: Some(2) }))
            .await
            .0;
        assert_eq!(capped.matched, 6, "limit must not change the match count");
        assert_eq!(capped.returned, 2);
        assert_eq!(capped.alphas.len(), 2);

        let filtered = s
            .list_alphas(Parameters(ListAlphasParams { filter: Some("BARK".into()), limit: None }))
            .await
            .0;
        assert_eq!(filtered.matched, 2, "the filter is not case-insensitive");
        assert_eq!(filtered.total, 6);

        let none = s
            .list_alphas(Parameters(ListAlphasParams { filter: Some("zzz".into()), limit: None }))
            .await
            .0;
        assert_eq!(none.matched, 0);
        assert!(none.alphas.is_empty());
    }

    #[tokio::test]
    async fn an_unknown_alpha_names_the_library() {
        let s = server();
        let err = s
            .add_tiling_layer(Parameters(AddTilingParams {
                alpha: "bark".into(),
                ..blank_tiling("bark")
            }))
            .await;
        let err = expect_err(err);
        let msg = err.message.to_string();
        assert!(msg.contains("Bark rough") || msg.contains("bark fine"), "{msg}");
        assert!(s.engine.lock().design().layers.layers.is_empty());
    }

    #[tokio::test]
    async fn a_seat_pad_defaults_onto_the_crest() {
        let s = server();
        let crest = s.engine.lock().design().field_context().crest_v_mm;
        s.add_seat_pad_layer(Parameters(AddSeatPadParams::default())).await.unwrap();
        let e = s.engine.lock();
        let Layer::SeatPad(pad) = &e.design().layers.layers[0].layer else {
            panic!("wrong layer kind");
        };
        assert!((pad.v_mm - crest).abs() < 1e-9, "pad sat at {} not {crest}", pad.v_mm);
    }

    #[tokio::test]
    async fn castability_carries_the_stones_section_when_seats_exist() {
        let s = server();
        let bare = s.castability().await;
        assert!(bare.0.stones.is_none(), "no seats yet");

        s.add_seat_pad_layer(Parameters(AddSeatPadParams::default())).await.unwrap();
        {
            let mut e = s.engine.lock();
            if let Layer::SeatPad(pad) = &mut e.design_mut().layers.layers[0].layer {
                pad.fit_stone(ringdesign_core::gem::Gem::calibrated(
                    ringdesign_core::gem::GemCut::Round,
                    3.0,
                ));
            }
        }
        let cast = s.castability().await;
        let stones = cast.0.stones.expect("stones section");
        assert_eq!(stones.stone_count, 1);
        assert!(stones.total_carats > 0.0);
        assert_eq!(stones.seats.len(), 1);
        assert!(stones.seats[0].stone.is_some());
        assert!(stones.seats[0].depth_available_mm > 0.0);
    }

    /// The signet head is base geometry, so setting it has to change the band
    /// itself — no layer involved — and picking an outline has to size the face
    /// to that shape rather than restretch the last one.
    #[tokio::test]
    async fn a_signet_head_shapes_the_band_and_sizes_itself_to_its_outline() {
        use ringdesign_core::profile::ShankKind;
        let s = server();
        let change = s
            .set_shank(Parameters(SetShankParams {
                kind: Some("signet".into()),
                amount: Some(0.85),
                head_outline: Some("cushion".into()),
                head_rise_mm: Some(1.0),
                ..SetShankParams::default()
            }))
            .await
            .unwrap()
            .0;
        assert!(change.applied.iter().any(|a| a.contains("Cushion")), "{:?}", change.applied);

        let e = s.engine.lock();
        let d = e.design();
        assert!(d.layers.layers.is_empty(), "the head should not be a layer");
        assert_eq!(d.shank.kind, ShankKind::Signet);
        assert_eq!(d.shank.head.outline, SignetOutline::Cushion);
        let want = d.profile.width_mm * SignetOutline::Cushion.head_aspect();
        assert!(
            (d.shank.head.length_mm - want).abs() < 1e-9,
            "face is {:.2} mm long, not the {want:.2} mm a cushion wants",
            d.shank.head.length_mm
        );

        // The band really is wider and deeper at the head than behind it.
        let (inner_r, crest_r) = (d.inner_radius_mm(), d.reference_loop().crest_radius_mm);
        let head = d.shank.head_at(d.shank.head.theta_deg, inner_r, crest_r);
        let back = d.shank.head_at(d.shank.head.theta_deg + 180.0, inner_r, crest_r);
        assert!(head.outer_r > back.outer_r + 0.9, "{:?} vs {:?}", head.outer_r, back.outer_r);
        assert!(
            d.shank.signet_width_frac(d.shank.head.theta_deg + 180.0, inner_r, crest_r) < 0.3
        );
    }

    #[tokio::test]
    async fn a_signet_defaults_onto_the_crest_and_updates_by_index() {
        let s = server();
        let crest = s.engine.lock().design().field_context().crest_v_mm;
        s.add_signet_layer(Parameters(AddSignetParams {
            outline: Some("cushion".into()),
            length_mm: Some(14.0),
            ..AddSignetParams::default()
        }))
        .await
        .unwrap();
        {
            let e = s.engine.lock();
            let Layer::Signet(sig) = &e.design().layers.layers[0].layer else {
                panic!("wrong layer kind");
            };
            assert!((sig.v_mm - crest).abs() < 1e-9, "table sat at {} not {crest}", sig.v_mm);
            assert_eq!(sig.outline, SignetOutline::Cushion);
            assert_eq!(sig.length_mm, 14.0);
            assert_eq!(e.design().layers.layers[0].layer.kind_label(), "Signet");
        }

        s.update_layer(Parameters(UpdateLayerParams {
            outline: Some("Hexagon".into()),
            top_flat: Some(0.5),
            shoulder_mm: Some(2.0),
            ..blank_update(0)
        }))
        .await
        .unwrap();
        let e = s.engine.lock();
        let Layer::Signet(sig) = &e.design().layers.layers[0].layer else {
            panic!("wrong layer kind");
        };
        assert_eq!(sig.outline, SignetOutline::Hexagon);
        assert_eq!((sig.top_flat, sig.shoulder_mm), (0.5, 2.0));
    }

    #[tokio::test]
    async fn signet_fields_are_rejected_on_other_kinds_and_the_outline_is_validated() {
        let s = server();
        s.add_milgrain_layer(Parameters(AddMilgrainParams::default())).await.unwrap();
        let err =
            expect_err(s.update_layer(Parameters(UpdateLayerParams { top_flat: Some(0.5), ..blank_update(0) })).await);
        let msg = err.message.to_string();
        assert!(msg.contains("Milgrain") && msg.contains("top_flat"), "{msg}");

        let err = expect_err(
            s.add_signet_layer(Parameters(AddSignetParams {
                outline: Some("trapezoid".into()),
                ..AddSignetParams::default()
            }))
            .await,
        );
        assert!(err.message.contains("Hexagon"), "{}", err.message);
        // The rejected call left nothing behind.
        assert_eq!(s.engine.lock().design().layers.layers.len(), 1);
    }

    #[tokio::test]
    async fn a_cross_section_is_downsampled_and_keeps_its_summary() {
        let s = server();
        let out = s
            .cross_section(Parameters(SectionParams { theta_deg: Some(90.0), steps: Some(512) }))
            .await
            .0;
        assert_eq!(out.sampled_points, 512);
        assert!(out.returned_points <= SECTION_POINT_CAP, "{}", out.returned_points);
        assert!(out.returned_points > 60, "{}", out.returned_points);
        assert!(out.max_r_mm > out.min_r_mm && out.min_wall_mm > 0.0);
        assert_eq!(out.undercut_count, 0, "a bare band should not undercut");
    }

    #[tokio::test]
    async fn a_full_pass_describes_builds_reports_and_exports() {
        let s = server();
        s.set_build_params(Parameters(SetBuildParamsParams {
            theta_steps: Some(96),
            profile_steps: Some(64),
        }))
        .await
        .unwrap();
        s.add_milgrain_layer(Parameters(AddMilgrainParams {
            beads_around: Some(72),
            ..Default::default()
        }))
        .await
        .unwrap();

        let d = s.describe_ring().await.0;
        assert_eq!(d.layers.len(), 1);
        assert!(d.silver_925_g > 0.5 && d.gold_14k_g > d.silver_925_g);
        assert!(d.crest_v_mm > 0.0 && d.crest_v_mm < d.band_v_len_mm);
        assert!(d.summary.contains("US 7"), "{}", d.summary);

        let r = s.build(Parameters(BuildParamsOverride::default())).await.0;
        assert!(r.watertight, "{r:?}");
        assert_eq!(r.triangle_count, 96 * 64 * 2);
        assert!(r.max_relief_mm > 0.0);
        assert_eq!(r.metals.len(), 10);

        let c = s.castability().await.0;
        assert!(!c.notes.is_empty());
        assert!(c.total_area_mm2 > 0.0);
        assert_eq!(c.good_faces + c.marginal_faces + c.vertical_faces + c.undercut_faces, r.triangle_count);

        let dir = std::env::temp_dir().join("ringdesign_mcp_test");
        std::fs::create_dir_all(&dir).unwrap();
        let stl = dir.join("pass.stl");
        let out = s
            .export_stl(Parameters(ExportParams {
                path: Some(stl.to_string_lossy().into_owned()),
            }))
            .await
            .unwrap()
            .0;
        assert!(out.bytes > 84);

        let json = dir.join("pass.ring.json");
        let path = json.to_string_lossy().into_owned();
        s.save_design(Parameters(PathParams { path: path.clone() })).await.unwrap();
        let other = server();
        other.load_design(Parameters(PathParams { path })).await.unwrap();
        assert_eq!(other.engine.lock().design().layers.layers.len(), 1);

        let _ = std::fs::remove_file(&stl);
        let _ = std::fs::remove_file(&json);
    }

    #[test]
    fn the_router_carries_every_tool_with_an_object_schema() {
        let tools = RingDesignServer::tool_router().list_all();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        for expected in [
            "get_design",
            "describe_ring",
            "list_profile_styles",
            "list_shank_styles",
            "list_alphas",
            "list_layers",
            "set_ring",
            "set_profile",
            "set_shank",
            "set_casting",
            "set_build_params",
            "add_tiling_layer",
            "add_signet_layer",
            "add_border_layer",
            "add_seat_pad_layer",
            "add_milgrain_layer",
            "update_layer",
            "remove_layer",
            "move_layer",
            "set_layer_enabled",
            "clear_layers",
            "build",
            "castability",
            "cross_section",
            "export_stl",
            "export_obj",
            "export_3mf",
            "export_glb",
            "save_design",
            "load_design",
            "apply_template",
        ] {
            assert!(names.contains(&expected), "missing tool {expected}: {names:?}");
        }
        assert_eq!(names.len(), 32, "{names:?}");
        for t in &tools {
            let d = t.description.as_ref().unwrap_or_else(|| panic!("{} has no description", t.name));
            assert!(d.len() > 120, "{} has a thin description", t.name);
            assert_eq!(
                t.input_schema.get("type").and_then(|v| v.as_str()),
                Some("object"),
                "{} input schema is not an object",
                t.name
            );
            assert!(t.output_schema.is_some(), "{} has no output schema", t.name);
        }
    }

    #[tokio::test]
    async fn params_deserialize_from_an_empty_object() {
        let p: SetProfileParams = serde_json::from_str("{}").unwrap();
        assert!(p.width_mm.is_none() && p.style.is_none());
        let p: ListAlphasParams = serde_json::from_str("{}").unwrap();
        assert!(p.limit.is_none());
        let p: AddTilingParams = serde_json::from_str(r#"{"alpha":"Rope"}"#).unwrap();
        assert_eq!(p.alpha, "Rope");
        assert!(p.repeats_around.is_none());
    }

    /// An `UpdateLayerParams` with nothing set but the index.
    fn blank_update(index: usize) -> UpdateLayerParams {
        UpdateLayerParams { index, ..Default::default() }
    }


    /// An `AddTilingParams` with nothing set but the alpha.
    fn blank_tiling(alpha: &str) -> AddTilingParams {
        AddTilingParams { alpha: alpha.into(), ..Default::default() }
    }

}
