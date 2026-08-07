//! MCP prompts: the sand-casting workflows, written against the real tool
//! names and real numbers.
//!
//! Each prompt returns one user message. The knowledge that makes a design
//! castable — crown against relief height, integer repeat counts, where in `v`
//! a pattern survives — lives in these bodies rather than in the agent.

use rmcp::ErrorData;
use rmcp::model::{
    GetPromptResult, JsonObject, ListPromptsResult, Prompt, PromptArgument, PromptMessage, Role,
};
use serde_json::Value;

/// The four workflows this server offers.
pub fn list() -> ListPromptsResult {
    ListPromptsResult::with_all_items(vec![
        Prompt::new(
            "design_castable_band",
            Some(
                "Design a patterned band that releases from a two-part sand mould: pick a profile \
                 with real crown, tile a motif onto it with an integer repeat count, keep the \
                 relief shallow near the crest, then check castability and iterate on the notes.",
            ),
            Some(vec![
                PromptArgument::new("motif")
                    .with_description(
                        "What the band should look like — rope, braid, basketweave, bark, \
                         chevron, hammered. Matched against the alpha library.",
                    )
                    .with_required(true),
                PromptArgument::new("ring_size")
                    .with_description("US finger size, quarter sizes allowed. Defaults to whatever the design already has.")
                    .with_required(false),
                PromptArgument::new("width_mm")
                    .with_description("Axial band width in mm. 4 to 8 is usual; wider bands need more crown to stay castable.")
                    .with_required(false),
            ]),
        ),
        Prompt::new(
            "diagnose_undercuts",
            Some(
                "Read the castability report and clear the undercuts in priority order: lower the \
                 relief, raise the crown, move the pattern toward the side faces, add side draft.",
            ),
            None,
        ),
        Prompt::new(
            "fit_pattern_to_band",
            Some(
                "Choose an integer repeat count so a tiled alpha closes at the seam, and match \
                 the cell aspect so the motif is not stretched.",
            ),
            Some(vec![
                PromptArgument::new("alpha")
                    .with_description("Name of an alpha from list_alphas or ring://alphas.")
                    .with_required(true),
                PromptArgument::new("repeats")
                    .with_description("Starting tile count around the ring. Integer. Omit to derive it from the cell aspect.")
                    .with_required(false),
            ]),
        ),
        Prompt::new(
            "prepare_for_casting",
            Some(
                "Final check before the caster sees it: watertight mesh, minimum section, zero \
                 undercut area, metal weight, then export at full resolution.",
            ),
            None,
        ),
    ])
}

/// Build one prompt's message from its arguments.
pub fn get(name: &str, args: Option<&JsonObject>) -> Result<GetPromptResult, ErrorData> {
    let body = match name {
        "design_castable_band" => design_castable_band(args)?,
        "diagnose_undercuts" => diagnose_undercuts(),
        "fit_pattern_to_band" => fit_pattern_to_band(args)?,
        "prepare_for_casting" => prepare_for_casting(),
        other => {
            return Err(ErrorData::invalid_params(
                format!(
                    "no prompt named {other:?}. Known: design_castable_band, diagnose_undercuts, \
                     fit_pattern_to_band, prepare_for_casting."
                ),
                None,
            ));
        }
    };
    Ok(GetPromptResult::new(vec![PromptMessage::new_text(Role::User, body)]))
}

fn design_castable_band(args: Option<&JsonObject>) -> Result<String, ErrorData> {
    let motif = require(args, "motif", "design_castable_band")?;
    let size = arg(args, "ring_size")
        .map(|s| format!("Set the size to US {s}."))
        .unwrap_or_else(|| "Keep the size the design already has; describe_ring reports it.".into());
    let width = arg(args, "width_mm")
        .map(|w| format!("Target width {w} mm."))
        .unwrap_or_else(|| "Pick a width between 4 and 8 mm.".into());

    Ok(format!(
        "Design a band with a {motif} motif that releases from a two-part sand mould. {size} \
         {width}\n\n\
         The mould parts on a plane perpendicular to the finger axis and pulls in BOTH \
         directions. Relief survives only where the outer surface still domes away from its \
         crest. On the crest line itself it undercuts almost immediately — about 0.05 mm of \
         usable relief on a half-round band — while the same texture 1 to 2 mm out on the flanks \
         is fine. The two annular side faces have perfect draft and always pull.\n\n\
         Work in this order.\n\n\
         1. describe_ring. Record circumference_mm (the `u` span, arc distance around the ring), \
         band_v_len_mm (the `v` span across the cross-section) and crest_v_mm (where the crest \
         sits in `v`). Every layer is positioned in those two coordinates, in mm.\n\n\
         2. Shape the band: set_ring, then set_profile {{ style, width_mm, thickness_mm }}. Choose \
         a profile with real crown, because crown is what buys the flanks their draft — HighDome \
         sets crown to 1.00 x thickness, HalfRound 0.90, CushionDome 0.70, DShape 0.65. Flat sets \
         0.15 and its outer wall is near vertical: put nothing on it but side-face relief. A \
         normal band is thickness_mm 1.8 to 2.4. Read crown_mm back out of the response — it is \
         clamped so the outer edge keeps 0.2 mm of metal, so a wide thin band silently gets less \
         crown than you asked for.\n\n\
         3. Pick the alpha: list_alphas {{ filter: \"{motif}\" }} (or read ring://alphas), then \
         read ring://alpha/<name> for its aspect ratio before you choose cell counts.\n\n\
         4. add_tiling_layer. repeats_around MUST be an integer: `u` wraps at the circumference, \
         so an integer tile count closes the pattern on itself by construction — no seam to hide. \
         The same is true of milgrain beads_around and border rope_twists. Size the cell as \
         cell_u = circumference_mm / repeats_around and cell_v = v_span_mm / rows, and keep \
         cell_u / cell_v near the alpha's aspect or the motif comes out stretched. Start \
         conservative: height_mm 0.30, v_center_mm = crest_v_mm, v_span_mm about 0.6 x \
         band_v_len_mm, feather_mm 0.4, continuous true.\n\n\
         5. castability. Read `notes` verbatim — they are written for this and name the fix. Then \
         cross_section {{ theta_deg: 90 }} and find the points whose class is \"Undercut\", which \
         tells you where in the section the damage is rather than only how much.\n\n\
         6. Iterate, cheapest fix first, re-running castability after each: lower height_mm with \
         update_layer; raise crown_mm with set_profile; move v_center_mm off the crest toward a \
         side face or narrow v_span_mm; add side_draft_deg 3 to 5. Moving the pattern onto the \
         flanks is what fixes it most of the time.\n\n\
         7. When the verdict is \"Castable\": set_build_params {{ theta_steps: 1024, \
         profile_steps: 320 }}, build and confirm watertight, then export_stl."
    ))
}

fn diagnose_undercuts() -> String {
    "The current design has undercuts. Find them and clear them.\n\n\
     1. Read ring://castability (or call castability). What matters: verdict, undercut_faces, \
     undercut_area_mm2 against total_area_mm2, worst_draft_deg (how far the worst face leans back \
     under itself, in degrees, negative meaning it locks in the sand), and `notes` — the notes are \
     plain language and say what to cut and where.\n\n\
     2. Locate it before changing anything. cross_section {{ theta_deg: 90 }}, then 0 and 180 as \
     well if the shank is modulated. Points with class \"Undercut\" and surface true are height \
     field damage. Points with class \"Vertical wall\" at small radius are the finger hole and are \
     never a problem — the bore cores in the sand or gets reamed at the bench.\n\n\
     3. Apply fixes in this order, re-running castability after each and stopping at the first \
     that clears it.\n\n\
     a. Lower the relief: update_layer {{ index, height_mm }}. Halve it. On a half-round crown \
     about 0.05 mm is all the crest line will take, while 0.3 to 0.5 mm is fine out on the flanks. \
     This is the fix in most cases and it costs nothing but depth.\n\n\
     b. Raise the crown: set_profile {{ crown_mm }} toward 0.9 to 1.0 x thickness_mm, or \
     set_profile {{ style: \"HighDome\" }}. A steeper dome means every point on the flank already \
     leans outward, so the same relief still nets positive draft. Crown is clamped to keep 0.2 mm \
     of metal at the edge, so check what you actually got.\n\n\
     c. Move the pattern off the crest: update_layer {{ index, v_center_mm }} toward a side face, \
     or shrink v_span_mm. The side faces have perfect draft — relief there pulls straight out no \
     matter how tall it is.\n\n\
     d. Add side draft: set_profile {{ side_draft_deg: 3 to 5 }}. Positive narrows the band \
     outward and lifts marginal side faces into good draft. It does nothing for an undercut on the \
     crown.\n\n\
     e. Last, soften the alpha itself: raise `bias` or lower `contrast` on the tiling layer to \
     flatten its steepest walls.\n\n\
     4. If undercut_faces reaches 0 but marginal stays high, the mould will drag rather than lock. \
     Keep going with (b) and (d), or decide the tolerance deliberately with set_casting \
     {{ min_draft_deg }}.\n\n\
     5. Finish with build and confirm watertight is true, then re-read ring://castability to \
     record the final verdict."
        .to_string()
}

fn fit_pattern_to_band(args: Option<&JsonObject>) -> Result<String, ErrorData> {
    let alpha = require(args, "alpha", "fit_pattern_to_band")?;
    let start = match arg(args, "repeats") {
        Some(r) => format!(
            "Start from repeats_around = {r} and check the cell it produces in step 3; move to \
             the nearest integer that fixes the aspect."
        ),
        None => "Derive repeats_around from the cell aspect in step 3.".to_string(),
    };

    Ok(format!(
        "Fit the alpha \"{alpha}\" to the band so it closes at the seam and is not stretched.\n\n\
         1. describe_ring for circumference_mm (the `u` span at the crest radius), band_v_len_mm \
         (the `v` span across the cross-section) and crest_v_mm. Read ring://alpha/{alpha} for the \
         alpha's pixel width, height and aspect.\n\n\
         2. repeats_around must be an INTEGER. `u` wraps at the circumference, so an integer tile \
         count closes the pattern on itself by construction — there is no join to hide and no \
         fudge factor. Fractional counts are not even expressible: repeats_around, milgrain \
         beads_around and border rope_twists are all u32 for exactly this reason. {start}\n\n\
         3. Match the cell to the alpha's aspect:\n\
         \x20     cell_u = circumference_mm / repeats_around\n\
         \x20     cell_v = v_span_mm / rows\n\
         Aim for cell_u / cell_v close to the alpha's aspect (width / height). Solved the other \
         way:\n\
         \x20     repeats_around = round( circumference_mm * rows / (v_span_mm * aspect) )\n\
         Round to the nearest integer and accept the few percent of scale change. Never stretch \
         v_span_mm to keep a fractional repeat — the seam matters more than the scale.\n\n\
         4. Place the strip in `v`. Starting point: v_center_mm = crest_v_mm, v_span_mm = 0.6 x \
         band_v_len_mm. Biasing v_center_mm away from crest_v_mm buys castable relief height: the \
         crest line takes about 0.05 mm before it undercuts, the flanks take several times that.\n\n\
         5. add_tiling_layer {{ alpha: \"{alpha}\", repeats_around, rows, v_center_mm, v_span_mm, \
         height_mm }} with continuous: true so a seamless source keeps flowing across cell \
         boundaries instead of being clamped per cell, feather_mm 0.3 to 0.5 so the tiling fades \
         out at the `v` edges rather than ending on a wall, and gap_mm 0 unless you want flat metal \
         between tiles. stagger 0.5 gives a brick lay; mirror_alternate_u makes a directional \
         motif read symmetrically.\n\n\
         6. Verify. build, then castability. Then cross_section at theta_deg 0 and 90: the relief \
         should reach the height_mm you set at both, and the tiling should still sit where you put \
         it in `v`. If a shank taper has moved it, narrow v_span_mm. list_layers gives the index \
         for update_layer if you need to re-tune without rebuilding the stack."
    ))
}

fn prepare_for_casting() -> String {
    "Final check before this design goes to the caster. Run every step; never export off a stale \
     build.\n\n\
     1. build. Require watertight true with boundary_edges 0 and non_manifold_edges 0. The mesh is \
     a torus grid closed in both directions, so it is watertight by construction — a failure means \
     a degenerate parameter (zero width, zero thickness, a resolution below 24), not a hole to \
     patch.\n\n\
     2. castability. Require verdict \"Castable\". \"Marginal\" means it drags on the sand and the \
     caster loses detail; \"NotCastable\" means it locks and the mould breaks getting it out. If it \
     is not Castable, run the diagnose_undercuts workflow and come back.\n\n\
     3. Minimum section. cross_section at theta_deg 0, 90, 180 and 270. min_wall_mm on each slice \
     is the thinnest metal between the outer surface and the bore. Compare it against the \
     min_section_mm in set_casting: below 0.5 mm the pour will not fill reliably, and the profile \
     already clamps outer edges to 0.2 mm because feather edges do not fill at all.\n\n\
     4. Undercut area. Even at verdict \"Castable\", confirm undercut_area_mm2 is 0 and \
     worst_draft_deg is positive. A few marginal faces on the side walls are survivable; any \
     undercut on the outer surface is not.\n\n\
     5. Weight. The build report's `metals` gives grams and pennyweight in ten alloys. Sand casting \
     needs a sprue and a feeder on top of the part, so budget roughly 1.5 to 2 x the part weight of \
     metal per pour.\n\n\
     6. Export at full resolution. set_build_params {{ theta_steps: 1024, profile_steps: 320 }}, \
     build once more and confirm watertight again because the resolution changed the mesh, then \
     export_stl. Use export_obj only for renders — the caster wants STL.\n\n\
     7. save_design next to the STL so the parameters survive. The file references alphas by name \
     rather than embedding them, so rebuilding it identically needs the same library."
        .to_string()
}

/// A prompt argument as a trimmed string. Numbers are accepted as well as strings.
fn arg(args: Option<&JsonObject>, key: &str) -> Option<String> {
    match args?.get(key)? {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn require(args: Option<&JsonObject>, key: &str, prompt: &str) -> Result<String, ErrorData> {
    arg(args, key).ok_or_else(|| {
        ErrorData::invalid_params(format!("{prompt} requires a non-empty `{key}` argument"), None)
    })
}

/* ---------------------------------------------------------------------------
PASTE INTO `#[tool_handler] impl ServerHandler for RingDesignServer` in tools.rs
if the prompt methods are not already there. Needs, in that file:

    use rmcp::model::{
        GetPromptRequestParams, GetPromptResponse, ListPromptsResult, PaginatedRequestParams,
    };
    use rmcp::service::{RequestContext, RoleServer};

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        Ok(crate::prompts::list())
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, ErrorData> {
        crate::prompts::get(&request.name, request.arguments.as_ref()).map(Into::into)
    }
--------------------------------------------------------------------------- */

#[cfg(test)]
mod tests {
    use rmcp::model::ContentBlock;
    use serde_json::json;

    use super::*;

    fn text_of(result: &GetPromptResult) -> String {
        match &result.messages[0].content {
            ContentBlock::Text(t) => t.text.clone(),
            other => panic!("expected text content, got {other:?}"),
        }
    }

    fn args(pairs: &[(&str, Value)]) -> JsonObject {
        pairs.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect()
    }

    #[test]
    fn every_advertised_prompt_resolves_with_its_required_arguments() {
        for p in list().prompts {
            let supplied: JsonObject = p
                .arguments
                .clone()
                .unwrap_or_default()
                .iter()
                .filter(|a| a.required == Some(true))
                .map(|a| (a.name.clone(), json!("rope")))
                .collect();
            let result = get(&p.name, Some(&supplied)).unwrap_or_else(|e| panic!("{}: {e}", p.name));
            assert!(text_of(&result).len() > 400, "{} is too thin to be useful", p.name);
        }
    }

    #[test]
    fn a_missing_required_argument_is_an_error_not_a_hole_in_the_text() {
        assert!(get("design_castable_band", None).is_err());
        assert!(get("fit_pattern_to_band", Some(&args(&[("alpha", json!(" "))]))).is_err());
    }

    #[test]
    fn arguments_land_in_the_body_and_numbers_are_accepted() {
        let a = args(&[("motif", json!("braid")), ("ring_size", json!(9.5)), ("width_mm", json!(6))]);
        let body = text_of(&get("design_castable_band", Some(&a)).unwrap());
        assert!(body.contains("braid"));
        assert!(body.contains("US 9.5"));
        assert!(body.contains("6 mm"));
    }

    #[test]
    fn the_repeat_count_advice_says_integer_and_why() {
        let a = args(&[("alpha", json!("Rope")), ("repeats", json!(28))]);
        let body = text_of(&get("fit_pattern_to_band", Some(&a)).unwrap());
        assert!(body.contains("repeats_around = 28"));
        assert!(body.contains("INTEGER"));
        assert!(body.contains("wraps at the circumference"));
    }

    #[test]
    fn the_prompts_name_real_tools() {
        for (name, tools) in [
            ("design_castable_band", ["set_profile", "add_tiling_layer", "castability"]),
            ("diagnose_undercuts", ["update_layer", "set_profile", "cross_section"]),
            ("fit_pattern_to_band", ["describe_ring", "add_tiling_layer", "list_layers"]),
            ("prepare_for_casting", ["build", "set_build_params", "export_stl"]),
        ] {
            let a = args(&[("motif", json!("rope")), ("alpha", json!("Rope"))]);
            let body = text_of(&get(name, Some(&a)).unwrap());
            for tool in tools {
                assert!(body.contains(tool), "{name} never mentions {tool}");
            }
        }
    }

    #[test]
    fn an_unknown_prompt_lists_the_known_ones() {
        let err = get("make_me_a_ring", None).expect_err("should not resolve");
        assert!(err.message.contains("design_castable_band"));
    }
}
