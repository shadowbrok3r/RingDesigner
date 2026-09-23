//! The one request a worker reads from its stdin and the one response it writes back, as JSON.
use ringdesign_core::cad::stored::Packed;
use serde::{Deserialize, Serialize};

/// The line a worker writes before its response, so whatever OpenCascade prints first is skipped.
pub const MARKER: &str = "@@ringdesign-occt-response@@";

/// How finely the worker tessellates what it builds.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Tolerance {
    /// Largest gap between a facet and the surface, mm.
    pub chord_mm: f64,
    /// Largest turn between neighbouring facets, degrees.
    pub angle_deg: f64,
}

impl Tolerance {
    /// Fine enough to cast from: the kernel's own export chord.
    pub const EXPORT: Self = Self { chord_mm: 0.015, angle_deg: 8.0 };
}

/// One of our edges as points along it, for the worker to find the same edge in its own reading of the part.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EdgeProbe {
    pub points: Vec<[f64; 3]>,
}

/// A frame: an origin and three unit axes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Frame {
    pub origin: [f64; 3],
    pub x: [f64; 3],
    pub y: [f64; 3],
    pub z: [f64; 3],
}

/// A torus round the finger's axis through the origin, as the kernel's `Torus` builds one.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Torus {
    pub major_mm: f64,
    pub minor_mm: f64,
}

/// A cylinder along its frame's z, centred on the frame's origin, as the kernel's `Cylinder` stands once seated.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Cylinder {
    pub radius_mm: f64,
    pub height_mm: f64,
    pub frame: Frame,
}

/// What a worker is asked to build.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    /// Answers with no solids: the worker starts and reads.
    Ping,
    /// The one solid `step` holds, `edges` found again by their points and rounded by `radius_mm`.
    Fillet { step: String, edges: Vec<EdgeProbe>, radius_mm: f64, tolerance: Tolerance },
    /// `torus` and `cylinder` fused, every edge where they meet rounded by `radius_mm`.
    Junction { torus: Torus, cylinder: Cylinder, radius_mm: f64, tolerance: Tolerance },
    /// The one solid `step` holds hollowed to walls `thickness_mm` thick, the faces nearest `open` left open.
    Shell { step: String, open: Vec<[f64; 3]>, thickness_mm: f64, tolerance: Tolerance },
    /// Every solid `step` holds.
    Import { step: String, tolerance: Tolerance },
}

impl Request {
    /// The operation's name, as a stored mesh's recipe records it.
    pub fn op(&self) -> &'static str {
        match self {
            Self::Ping => "ping",
            Self::Fillet { .. } => "fillet",
            Self::Junction { .. } => "junction",
            Self::Shell { .. } => "shell",
            Self::Import { .. } => "import",
        }
    }
}

/// One solid a worker built, tessellated and closed.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Built {
    pub name: String,
    /// OpenCascade's own volume of the solid, mm³.
    pub brep_volume_mm3: f64,
    /// The tessellation's volume, mm³.
    pub mesh_volume_mm3: f64,
    pub faces: usize,
    pub edges: usize,
    /// The tessellation, welded and closed, packed as the design file keeps it.
    pub mesh: Packed,
}

/// What a worker answers.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Response {
    /// The solids built, what the worker had to say about them, and how long OpenCascade took, ms.
    Done { solids: Vec<Built>, notes: Vec<String>, kernel_ms: f64 },
    /// OpenCascade, or the worker, refused: why.
    Refused { message: String },
}
