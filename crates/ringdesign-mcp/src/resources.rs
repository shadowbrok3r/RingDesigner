//! MCP resources: the design, its two reports, and the alpha library, addressed
//! as `ring://` URIs.
//!
//! Payloads are trimmed on the way out. The castability report carries a face
//! class per triangle and a raw cross-section carries a point per profile step;
//! both are summarised or downsampled here so a read costs an agent a few
//! hundred bytes rather than a few hundred kilobytes.

use ringdesign_core::alpha::Alpha;
use ringdesign_core::castability::Section;
use rmcp::ErrorData;
use rmcp::model::{
    ListResourceTemplatesResult, ListResourcesResult, ReadResourceResult, Resource,
    ResourceContents, ResourceTemplate,
};
use serde_json::{Value, json};

use crate::RingDesignServer;

const SCHEME: &str = "ring://";
const JSON_MIME: &str = "application/json";

/// Longest edge of the ASCII preview in `ring://alpha/{name}`.
const PREVIEW_EDGE: usize = 32;

/// Darkest-to-brightest ramp the alpha preview is drawn with.
const PREVIEW_RAMP: &[u8] = b" .:-=+*#%@";

/// Cross-section sampling density for `ring://section/{theta}`.
const SECTION_STEPS: usize = 192;

/// Most section points `ring://section/{theta}` returns.
const SECTION_POINT_CAP: usize = 96;

/// Most rows `ring://alphas` returns.
const ALPHA_INDEX_CAP: usize = 256;

/// The four fixed resources, all JSON.
pub fn list() -> ListResourcesResult {
    ListResourcesResult::with_all_items(vec![
        Resource::new("ring://design", "design")
            .with_title("Current design")
            .with_mime_type(JSON_MIME)
            .with_description(
                "The whole RingDesign as stored — size, profile, shank, layer stack, build \
                 resolution and casting settings — plus the engine generation counter. Same shape \
                 as a saved .ring.json file. The generation increments on every mutation, so poll \
                 it to notice edits made in the GUI.",
            ),
        Resource::new("ring://report", "report")
            .with_title("Build report")
            .with_mime_type(JSON_MIME)
            .with_description(
                "The build report, rebuilding first if the design changed: watertight check with \
                 boundary and non-manifold edge counts, triangle and vertex counts, volume in \
                 mm3, surface area, overall size, inner and outer diameter, band width, the \
                 tallest and deepest displacement the layer stack applied, and cast weight in ten \
                 metals.",
            ),
        Resource::new("ring://castability", "castability")
            .with_title("Castability report")
            .with_mime_type(JSON_MIME)
            .with_description(
                "Verdict and draft analysis for a two-part sand mould parting perpendicular to \
                 the finger axis and pulling both ways: face counts and areas per class, the \
                 undercut share of the surface, the worst draft angle found (negative leans back \
                 under itself and locks in the sand), the parting height, and the notes. The \
                 per-triangle class array is omitted — call cross_section to locate an undercut.",
            ),
        Resource::new("ring://graph", "graph")
            .with_title("The design's graph")
            .with_description("The dataflow graph behind the current design as JSON (null when the design is hand-made); the graph_* tools edit it and graph_evaluate runs it.")
            .with_mime_type("application/json"),
        Resource::new("ring://graph/nodes", "graph nodes")
            .with_title("Node library")
            .with_description("Every node kind the graph runtime knows, with pins, kinds, defaults and docs.")
            .with_mime_type("application/json"),
        Resource::new("ring://alphas", "alphas")
            .with_title("Alpha library index")
            .with_mime_type(JSON_MIME)
            .with_description(
                "Every alpha available to a tiling layer: name, pixel width and height. Names are \
                 what add_tiling_layer takes. Capped at 256 rows with `total` and `truncated` \
                 alongside — use list_alphas with a filter on a bigger library. Read \
                 ring://alpha/{name} for one entry's aspect ratio and a preview.",
            ),
    ])
}

/// The two parameterised resources.
pub fn templates() -> ListResourceTemplatesResult {
    ListResourceTemplatesResult::with_all_items(vec![
        ResourceTemplate::new("ring://alpha/{name}", "alpha")
            .with_title("One alpha")
            .with_mime_type(JSON_MIME)
            .with_description(
                "Metadata for a single alpha plus a small ASCII preview: pixel size, the \
                 width/height aspect to match a tile cell against so the motif is not stretched, \
                 the min, max and mean of the height samples, and a downsampled grayscale drawn \
                 with the ramp \" .:-=+*#%@\" (space is 0, @ is 1). Percent-encode spaces in the \
                 name.",
            ),
        ResourceTemplate::new("ring://section/{theta}", "section")
            .with_title("Cross-section at an angle")
            .with_mime_type(JSON_MIME)
            .with_description(
                "The displaced cross-section at a ring angle in degrees, 90 being the top of the \
                 ring: radial and axial extents, parting height, thinnest metal between the outer \
                 surface and the bore, undercut segment count, and the points themselves with \
                 signed draft and class. Evaluates the same profile and height field as the mesh \
                 build, so it never disagrees with the solid.",
            ),
    ])
}

impl RingDesignServer {
    /// Resolve a `ring://` URI to a single JSON body.
    pub(crate) fn read_ring_uri(&self, uri: &str) -> Result<ReadResourceResult, ErrorData> {
        let Some(rest) = uri.strip_prefix(SCHEME) else {
            return Err(not_found(uri));
        };
        let rest = rest.trim_end_matches('/');
        let value = match rest {
            "design" => self.design_value()?,
            "report" => self.report_value()?,
            "castability" => self.castability_value(),
            "alphas" => self.alphas_value(),
            "graph" => self.graph_value(),
            "graph/nodes" => self.node_kinds_value(),
            _ => {
                if let Some(name) = rest.strip_prefix("alpha/") {
                    self.alpha_value(&percent_decode(name), uri)?
                } else if let Some(theta) = rest.strip_prefix("section/") {
                    self.section_value(theta, uri)?
                } else {
                    return Err(not_found(uri));
                }
            }
        };
        Ok(ReadResourceResult::new(vec![json_contents(uri, &value)?]))
    }

    fn design_value(&self) -> Result<Value, ErrorData> {
        let e = self.engine.lock();
        let generation = e.generation();
        let design = serde_json::to_value(e.design())
            .map_err(|err| ErrorData::internal_error(format!("serialize design: {err}"), None))?;
        Ok(json!({ "generation": generation, "design": design }))
    }

    fn report_value(&self) -> Result<Value, ErrorData> {
        let mut e = self.engine.lock();
        let report = e.report();
        let params = e.design().build;
        let generation = e.generation();
        drop(e);
        self.touch();
        let report = serde_json::to_value(&report)
            .map_err(|err| ErrorData::internal_error(format!("serialize report: {err}"), None))?;
        Ok(json!({
            "generation": generation,
            "theta_steps": params.theta_steps,
            "profile_steps": params.profile_steps,
            "report": report,
        }))
    }

    /// Everything in `CastReport` except the per-triangle class array.
    fn castability_value(&self) -> Value {
        let mut e = self.engine.lock();
        let cast = e.castability();
        let min_draft_deg = e.design().draft.min_draft_deg;
        let generation = e.generation();
        drop(e);
        self.touch();
        json!({
            "generation": generation,
            "verdict": format!("{:?}", cast.verdict),
            "verdict_label": cast.verdict.label(),
            "good_faces": cast.good,
            "marginal_faces": cast.marginal,
            "vertical_faces": cast.vertical,
            "undercut_faces": cast.undercut,
            "undercut_area_mm2": round(cast.undercut_area_mm2),
            "marginal_area_mm2": round(cast.marginal_area_mm2),
            "total_area_mm2": round(cast.total_area_mm2),
            "undercut_fraction": round(cast.undercut_fraction()),
            "worst_draft_deg": round(cast.worst_draft_deg),
            "parting_z_mm": round(cast.parting_z_mm),
            "min_draft_deg": min_draft_deg,
            "notes": cast.notes,
        })
    }

    fn alphas_value(&self) -> Value {
        let e = self.engine.lock();
        let total = e.library().len();
        let alphas: Vec<Value> = e
            .library()
            .iter()
            .take(ALPHA_INDEX_CAP)
            .map(|a| json!({ "name": a.name, "width": a.width, "height": a.height }))
            .collect();
        json!({
            "total": total,
            "returned": alphas.len(),
            "truncated": total > alphas.len(),
            "alphas": alphas,
        })
    }

    fn alpha_value(&self, name: &str, uri: &str) -> Result<Value, ErrorData> {
        let e = self.engine.lock();
        let alpha = e.library().get(name).ok_or_else(|| {
            let mut names = e.library().names();
            names.truncate(12);
            ErrorData::resource_not_found(
                format!(
                    "{uri}: no alpha named {name:?}. {} in the library, e.g. {}. Read ring://alphas \
                     for the index.",
                    e.library().len(),
                    names.join(", ")
                ),
                None,
            )
        })?;
        Ok(alpha_json(alpha))
    }

    fn section_value(&self, theta: &str, uri: &str) -> Result<Value, ErrorData> {
        let theta_deg: f64 = theta.parse().ok().filter(|t: &f64| t.is_finite()).ok_or_else(|| {
            ErrorData::resource_not_found(
                format!("{uri}: {theta:?} is not a ring angle in degrees, e.g. ring://section/90"),
                None,
            )
        })?;
        let e = self.engine.lock();
        let section = e.section(theta_deg, SECTION_STEPS);
        drop(e);
        Ok(section_json(&section))
    }
}

/// Metadata plus an ASCII grayscale preview.
fn alpha_json(alpha: &Alpha) -> Value {
    let (mut lo, mut hi, mut sum) = (f32::INFINITY, f32::NEG_INFINITY, 0.0f64);
    for &v in &alpha.data {
        lo = lo.min(v);
        hi = hi.max(v);
        sum += v as f64;
    }
    let n = alpha.data.len().max(1);
    let (pw, ph, rgba) = alpha.thumbnail_rgba8(PREVIEW_EDGE);
    let rows: Vec<String> = (0..ph)
        .map(|j| {
            (0..pw)
                .map(|i| {
                    let level = rgba.get((j * pw + i) * 4).copied().unwrap_or(0) as usize;
                    let idx = level * (PREVIEW_RAMP.len() - 1) / 255;
                    PREVIEW_RAMP[idx] as char
                })
                .collect()
        })
        .collect();
    json!({
        "name": alpha.name,
        "width": alpha.width,
        "height": alpha.height,
        "aspect": round(alpha.width as f64 / alpha.height.max(1) as f64),
        "min": round(lo.min(hi) as f64),
        "max": round(hi.max(lo) as f64),
        "mean": round(sum / n as f64),
        "preview": {
            "width": pw,
            "height": ph,
            "ramp": String::from_utf8_lossy(PREVIEW_RAMP),
            "rows": rows,
        },
    })
}

/// Summary fields plus every `stride`-th point of the slice.
fn section_json(section: &Section) -> Value {
    let sampled = section.points.len();
    let stride = sampled.div_ceil(SECTION_POINT_CAP).max(1);
    let points: Vec<Value> = section
        .points
        .iter()
        .step_by(stride)
        .map(|p| {
            json!({
                "r": round(p.r),
                "z": round(p.z),
                "draft_deg": round(p.draft_deg),
                "class": p.class.label(),
                "surface": p.surface,
            })
        })
        .collect();
    json!({
        "theta_deg": round(section.theta_deg),
        "parting_z_mm": round(section.parting_z_mm),
        "min_r_mm": round(section.min_r),
        "max_r_mm": round(section.max_r),
        "min_z_mm": round(section.min_z),
        "max_z_mm": round(section.max_z),
        "min_wall_mm": round(section.min_wall_mm),
        "undercut_count": section.undercut_count,
        "sampled_points": sampled,
        "returned_points": points.len(),
        "points": points,
    })
}

fn json_contents(uri: &str, value: &Value) -> Result<ResourceContents, ErrorData> {
    let text = serde_json::to_string(value)
        .map_err(|err| ErrorData::internal_error(format!("{uri}: {err}"), None))?;
    Ok(ResourceContents::text(text, uri).with_mime_type(JSON_MIME))
}

fn not_found(uri: &str) -> ErrorData {
    ErrorData::resource_not_found(
        format!(
            "{uri} is not a resource of this server. Known: ring://design, ring://report, \
             ring://castability, ring://alphas, ring://alpha/{{name}}, ring://section/{{theta}}."
        ),
        None,
    )
}

/// Three decimals, so a report reads in millimetres rather than float noise.
fn round(v: f64) -> f64 {
    if v.is_finite() { (v * 1e3).round() / 1e3 } else { 0.0 }
}

/// Decode `%XX` escapes in a URI path segment.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match (b[i], b.get(i + 1), b.get(i + 2)) {
            (b'%', Some(&h), Some(&l)) => match (hex(h), hex(l)) {
                (Some(h), Some(l)) => {
                    out.push(h << 4 | l);
                    i += 3;
                }
                _ => {
                    out.push(b[i]);
                    i += 1;
                }
            },
            _ => {
                out.push(b[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(c: u8) -> Option<u8> {
    (c as char).to_digit(16).map(|d| d as u8)
}

/* ---------------------------------------------------------------------------
PASTE INTO `#[tool_handler] impl ServerHandler for RingDesignServer` in tools.rs
if the resource methods are not already there. Needs, in that file:

    use rmcp::model::{
        ListResourceTemplatesResult, ListResourcesResult, PaginatedRequestParams,
        ReadResourceRequestParams, ReadResourceResponse,
    };
    use rmcp::service::{RequestContext, RoleServer};

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(crate::resources::list())
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        Ok(crate::resources::templates())
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        self.read_ring_uri(&request.uri).map(Into::into)
    }
--------------------------------------------------------------------------- */

#[cfg(test)]
mod tests {
    use ringdesign_core::alpha::{Alpha, AlphaLibrary};
    use ringdesign_core::{DesignEngine, SharedEngine};

    use super::*;

    fn engine() -> SharedEngine {
        let mut lib = AlphaLibrary::default();
        lib.insert(Alpha::new("Rope twist", 4, 2, vec![0.0, 0.25, 0.5, 1.0, 1.0, 0.5, 0.25, 0.0]));
        DesignEngine::shared(lib)
    }

    fn server() -> RingDesignServer {
        let s = RingDesignServer::new(engine());
        {
            let mut e = s.engine.lock();
            let d = e.design_mut();
            d.build.theta_steps = 64;
            d.build.profile_steps = 48;
        }
        s
    }

    fn read(s: &RingDesignServer, uri: &str) -> Value {
        let result = s.read_ring_uri(uri).expect("resource resolved");
        let text = match &result.contents[0] {
            ResourceContents::TextResourceContents { text, mime_type, .. } => {
                assert_eq!(mime_type.as_deref(), Some(JSON_MIME));
                text.clone()
            }
            other => panic!("expected text contents, got {other:?}"),
        };
        serde_json::from_str(&text).expect("body is JSON")
    }

    #[test]
    fn every_advertised_resource_reads() {
        let s = server();
        for r in list().resources {
            let v = read(&s, &r.uri);
            assert!(v.is_object(), "{} did not return an object", r.uri);
        }
    }

    #[test]
    fn design_carries_the_generation_and_the_whole_design() {
        let s = server();
        let v = read(&s, "ring://design");
        assert!(v["generation"].as_u64().is_some());
        assert_eq!(v["design"]["size"], serde_json::json!(7.0));
        assert!(v["design"]["profile"].is_object());
    }

    #[test]
    fn castability_omits_the_per_face_class_array() {
        let s = server();
        let v = read(&s, "ring://castability");
        assert!(v["classes"].is_null(), "the per-triangle array leaked into the payload");
        assert!(v["notes"].as_array().is_some_and(|n| !n.is_empty()));
        assert_eq!(v["verdict"], serde_json::json!("Castable"));
    }

    #[test]
    fn an_alpha_reads_by_percent_encoded_name() {
        let s = server();
        let v = read(&s, "ring://alpha/Rope%20twist");
        assert_eq!(v["name"], serde_json::json!("Rope twist"));
        assert_eq!(v["aspect"], serde_json::json!(2.0));
        let rows = v["preview"]["rows"].as_array().expect("preview rows");
        assert_eq!(rows.len(), 2);
        assert!(rows[0].as_str().is_some_and(|r| r.chars().count() == 4));
    }

    #[test]
    fn a_section_downsamples_and_reports_both_counts() {
        let s = server();
        let v = read(&s, "ring://section/90");
        assert_eq!(v["theta_deg"], serde_json::json!(90.0));
        let returned = v["returned_points"].as_u64().unwrap() as usize;
        assert!(returned <= SECTION_POINT_CAP);
        assert_eq!(v["points"].as_array().unwrap().len(), returned);
        assert!(v["sampled_points"].as_u64().unwrap() >= returned as u64);
    }

    #[test]
    fn unknown_uris_name_themselves_in_the_error() {
        let s = server();
        for uri in ["ring://nope", "file:///etc/passwd", "ring://alpha/Missing", "ring://section/x"]
        {
            let err = s.read_ring_uri(uri).expect_err("should not resolve");
            assert!(err.message.contains(uri), "{uri} missing from {:?}", err.message);
        }
    }

    #[test]
    fn templates_cover_both_parameterised_forms() {
        let uris: Vec<String> =
            templates().resource_templates.into_iter().map(|t| t.uri_template).collect();
        assert!(uris.contains(&"ring://alpha/{name}".to_string()));
        assert!(uris.contains(&"ring://section/{theta}".to_string()));
    }
}
