//! Shader sources, written once: [`Profile`] puts the version line and precision in front for
//! the context — GLSL 3.30 core on desktop GL, GLSL ES 3.00 on the phone and WebGL 2.

/// Which GLSL a context compiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// Desktop OpenGL 3.3 core.
    Core,
    /// OpenGL ES 3.0 and WebGL 2.
    Es,
}

impl Profile {
    /// Classifies a `GL_VERSION` string: ES and WebGL contexts compile GLSL ES 3.00, everything else 3.30 core.
    pub fn from_version(version: &str) -> Self {
        if version.contains("OpenGL ES") || version.contains("WebGL") { Self::Es } else { Self::Core }
    }

    /// The version line, and on ES the float precision a fragment shader has no default for.
    pub fn header(self) -> &'static str {
        match self {
            Self::Core => "#version 330 core\n",
            Self::Es => "#version 300 es\nprecision highp float;\nprecision highp int;\n",
        }
    }

    /// Whether `glPolygonMode` exists: desktop GL has it, no ES version does.
    pub fn has_polygon_mode(self) -> bool {
        self == Self::Core
    }

    /// A shader body with this profile's header in front.
    pub fn source(self, body: &str) -> String {
        format!("{}{body}", self.header())
    }
}

/// The metal's vertex stage: position, normal, draft and wall colours, the focus channel (4) and the select channel (5).
pub const MESH_VS: &str = r#"
layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec3 a_color;
layout(location = 3) in vec3 a_wall;
layout(location = 4) in float a_focus;
layout(location = 5) in vec2 a_select;

uniform mat4 u_mvp;
uniform mat3 u_normal_matrix;

out vec3 v_normal;
out vec3 v_color;
out vec3 v_wall;
out vec3 v_bary;
out float v_obj_nz;
out vec3 v_world;
out float v_cavity;
out float v_focus;
out vec2 v_select;

void main() {
    gl_Position = u_mvp * vec4(a_position, 1.0);
    v_focus = a_focus;
    v_select = a_select;
    v_normal = u_normal_matrix * a_normal;
    v_color = a_color;
    v_wall = a_wall;
    // Object-space axial share of the normal: which mould half owns the face.
    v_obj_nz = a_normal.z;
    v_world = a_position;
    // Approximate bore occlusion; assessment colours do not use it.
    v_cavity = smoothstep(0.0, 0.55, -dot(a_normal.xy, normalize(a_position.xy + vec2(0.000001))));
    // Non-indexed triangles: the corner is the vertex index mod 3.
    int corner = gl_VertexID % 3;
    v_bary = vec3(corner == 0 ? 1.0 : 0.0, corner == 1 ? 1.0 : 0.0, corner == 2 ? 1.0 : 0.0);
}
"#;

/// The metal's fragment stage, every shade mode; `// STUDIO_MATERIAL` takes the studio GLSL.
pub const MESH_FS: &str = r#"
in vec3 v_normal;
in vec3 v_color;
in vec3 v_wall;
in vec3 v_bary;
in float v_obj_nz;
in vec3 v_world;
in float v_cavity;
in float v_focus;
in vec2 v_select;
uniform vec4 u_clip_plane;
uniform vec4 u_focus;
uniform vec4 u_select;
uniform vec4 u_hover;
uniform float u_alpha;

uniform int u_mode;
uniform vec3 u_light_dir;
uniform vec3 u_base_color;
uniform float u_ambient;
uniform vec3 u_wire_color;
uniform float u_wire_px;

out vec4 frag_color;

// STUDIO_MATERIAL

void main() {
    if (dot(vec4(v_world, 1.0), u_clip_plane) > 0.00001) discard;
    vec3 n = normalize(v_normal);
    vec3 l = normalize(u_light_dir);
    vec3 color;

    if (u_mode == 6) {
        // Faint face on, bright where the surface turns away: a cutter reads as its outline.
        float rim = 1.0 - abs(n.z);
        frag_color = vec4(u_base_color, u_alpha * (0.16 + 0.84 * rim * rim));
        return;
    }
    if (u_mode == 5) {
        color = studio_gem(n, v_color, l, u_ambient);
    } else if (u_mode == 4) {
        // Cope in cool blue, drag in warm sand, the parting band bright.
        float lambert = max(dot(n, l), 0.0);
        vec3 half_c = v_obj_nz > 0.0 ? vec3(0.42, 0.62, 0.82) : vec3(0.80, 0.62, 0.38);
        float band = 1.0 - smoothstep(0.035, 0.09, abs(v_obj_nz));
        color = mix(half_c, vec3(1.0, 0.92, 0.25), band) * (0.72 + 0.28 * lambert);
    } else if (u_mode == 3) {
        float lambert = max(dot(n, l), 0.0);
        color = v_wall * (0.74 + 0.26 * lambert);
    } else if (u_mode == 2) {
        color = n * 0.5 + 0.5;
    } else if (u_mode == 1) {
        float lambert = max(dot(n, l), 0.0);
        color = v_color * (0.74 + 0.26 * lambert);
    } else {
        color = studio_metal(n, u_base_color, l, u_ambient, v_cavity);
    }

    // The chosen node's reach over whatever the mode drew, some key light kept so relief reads.
    float focus = clamp(v_focus, 0.0, 1.0) * u_focus.a;
    if (focus > 0.0) {
        float lit = 0.62 + 0.38 * max(dot(n, l), 0.0);
        color = mix(color, u_focus.rgb * lit, focus);
    }
    // The selection over that and the hover over both, crossfading rather than banding.
    float chosen = clamp(v_select.x, 0.0, 1.0) * u_select.a;
    float hovered = clamp(v_select.y, 0.0, 1.0) * u_hover.a;
    if (chosen > 0.0 || hovered > 0.0) {
        float lit = 0.62 + 0.38 * max(dot(n, l), 0.0);
        color = mix(color, u_select.rgb * lit, chosen);
        color = mix(color, u_hover.rgb * lit, hovered);
    }

    if (u_wire_px > 0.0) {
        vec3 w = fwidth(v_bary) * u_wire_px;
        vec3 a = smoothstep(vec3(0.0), w, v_bary);
        float edge = 1.0 - min(min(a.x, a.y), a.z);
        color = mix(color, u_wire_color, edge * 0.55);
    }

    frag_color = vec4(color, u_alpha);
}
"#;

/// The line wireframe's fragment stage, drawn through [`MESH_VS`] under `glPolygonMode(LINE)`.
pub const WIRE_FS: &str = r#"
in vec3 v_world;
uniform vec3 u_wire_color;
uniform vec4 u_clip_plane;

out vec4 frag_color;

void main() {
    if (dot(vec4(v_world, 1.0), u_clip_plane) > 0.00001) discard;
    frag_color = vec4(u_wire_color, 0.55);
}
"#;

/// The edge pass's vertex stage: each segment a quad widened on screen, lifted toward the eye.
pub const EDGE_VS: &str = r#"
layout(location = 0) in vec3 a_from;
layout(location = 1) in vec3 a_to;
layout(location = 2) in vec2 a_corner;
layout(location = 3) in float a_width;
layout(location = 4) in vec4 a_color;

uniform mat4 u_mvp;
uniform vec2 u_viewport;
uniform float u_px_per_pt;
uniform vec3 u_toward_eye;
uniform float u_bias_mm;

out vec4 v_color;
out float v_across;
out float v_half;
out vec3 v_world;

void main() {
    vec3 lift = u_toward_eye * u_bias_mm;
    vec4 a = u_mvp * vec4(a_from + lift, 1.0);
    vec4 b = u_mvp * vec4(a_to + lift, 1.0);
    vec2 half_view = 0.5 * u_viewport;
    vec2 d = (b.xy / b.w - a.xy / a.w) * half_view;
    float len = length(d);
    vec2 t = len > 0.0001 ? d / len : vec2(1.0, 0.0);
    vec2 n = vec2(-t.y, t.x);
    // Half the width, and a pixel more for the coverage ramp; the ends are squared off by as much.
    float half_px = 0.5 * a_width * u_px_per_pt;
    float reach = half_px + 1.0;
    vec4 p = mix(a, b, a_corner.x);
    vec2 offset = (n * a_corner.y + t * (2.0 * a_corner.x - 1.0)) * reach;
    p.xy += offset / half_view * p.w;
    gl_Position = p;
    v_color = a_color;
    v_across = a_corner.y * reach;
    v_half = half_px;
    v_world = mix(a_from, a_to, a_corner.x);
}
"#;

/// The edge pass's fragment stage: full coverage inside the width, a one-pixel ramp outside it.
pub const EDGE_FS: &str = r#"
in vec4 v_color;
in float v_across;
in float v_half;
in vec3 v_world;
uniform vec4 u_clip_plane;
uniform float u_opacity;

out vec4 frag_color;

void main() {
    if (dot(vec4(v_world, 1.0), u_clip_plane) > 0.00001) discard;
    float cover = clamp(v_half + 0.5 - abs(v_across), 0.0, 1.0);
    if (cover <= 0.0) discard;
    frag_color = vec4(v_color.rgb, v_color.a * cover * u_opacity);
}
"#;

/// [`MESH_FS`] with the studio material spliced in.
pub fn mesh_fs() -> String {
    MESH_FS.replace("// STUDIO_MATERIAL", ringdesign_core::render::STUDIO_GLSL)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(qualifier, type, name)` of every top-level `in`, `out` and `uniform` declaration.
    fn declared(src: &str) -> Vec<(String, String, String)> {
        src.lines()
            .map(|l| l.trim())
            .map(|l| l.rsplit_once(')').map_or(l, |(_, rest)| rest.trim()))
            .filter_map(|l| {
                let l = l.strip_suffix(';')?;
                let mut words = l.split_whitespace();
                let q = words.next()?;
                if !matches!(q, "in" | "out" | "uniform") {
                    return None;
                }
                let (ty, name) = (words.next()?, words.next()?);
                words.next().is_none().then(|| (q.to_string(), ty.to_string(), name.to_string()))
            })
            .collect()
    }

    fn varyings(src: &str, q: &str) -> Vec<(String, String)> {
        declared(src).into_iter().filter(|d| d.0 == q).map(|d| (d.1, d.2)).collect()
    }

    #[test]
    fn a_driver_version_string_picks_the_dialect() {
        for (version, want) in [
            ("4.6.0 NVIDIA 580.95.05", Profile::Core),
            ("4.6 (Core Profile) Mesa 25.2.3", Profile::Core),
            ("3.3.0 NVIDIA 470.256.02", Profile::Core),
            ("OpenGL ES 3.2 V@0615.65 (GIT@2e8b3a5, I8a1b2c)", Profile::Es),
            ("OpenGL ES 3.0 (4.6.0 NVIDIA 580.95.05)", Profile::Es),
            ("OpenGL ES 3.1 SwiftShader 4.0.0.1", Profile::Es),
            ("WebGL 2.0 (OpenGL ES 3.0 Chromium)", Profile::Es),
        ] {
            assert_eq!(Profile::from_version(version), want, "{version}");
        }
        assert_eq!(Profile::Core.header(), "#version 330 core\n");
        let es = Profile::Es.header();
        assert!(es.starts_with("#version 300 es\n") && es.contains("precision highp float;"), "{es}");
        assert!(Profile::Core.has_polygon_mode() && !Profile::Es.has_polygon_mode());
        assert!(Profile::Es.source(EDGE_FS).starts_with("#version 300 es\nprecision highp float;"));
        assert_eq!(Profile::Core.source(WIRE_FS).lines().next(), Some("#version 330 core"));
    }

    #[test]
    fn every_fragment_input_is_a_vertex_output_of_the_same_type() {
        let fs = mesh_fs();
        for (vs, fs, name) in [(MESH_VS, fs.as_str(), "mesh"), (MESH_VS, WIRE_FS, "wire"), (EDGE_VS, EDGE_FS, "edge")] {
            let outs = varyings(vs, "out");
            let ins = varyings(fs, "in");
            assert!(!ins.is_empty(), "{name}");
            for input in &ins {
                assert!(outs.contains(input), "{name}: fragment input {input:?} has no vertex output");
            }
        }
        assert_eq!(varyings(EDGE_VS, "in").len(), 5, "from, to, corner, width, colour");
    }

    #[test]
    fn the_studio_material_is_spliced_in_and_its_roughness_declared() {
        let fs = mesh_fs();
        assert!(!fs.contains("// STUDIO_MATERIAL"));
        assert!(fs.contains("vec3 studio_metal(") && fs.contains("vec3 studio_gem("));
        let uniforms: Vec<String> = declared(&fs).into_iter().filter(|d| d.0 == "uniform").map(|d| d.2).collect();
        for name in ["u_roughness", "u_mode", "u_wire_px", "u_focus", "u_select", "u_hover", "u_clip_plane", "u_alpha"] {
            assert!(uniforms.iter().any(|u| u == name), "{name} missing from {uniforms:?}");
        }
    }
}
