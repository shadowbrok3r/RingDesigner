//! Reproducible, editable construction steps for a curated sand signet.
//! A step changes ordinary design fields/layers; no finished mesh is loaded.
use crate::{RingDesign, field::LayerEntry};
use std::sync::OnceLock;

pub struct Step {
    pub title: &'static str,
    pub tool: &'static str,
    pub detail: &'static str,
    pub layers: &'static [&'static str],
}

pub static STEPS: &[Step] = &[
    Step {title: "Proportion the band", tool: "Profile & sizing", detail: "18.20 mm bore · 12.60 mm face width · 3.20 mm stock. Rounded inside edge and 4° side draft.", layers: &[]},
    Step {title: "Sculpt the cushion head", tool: "Signet & shoulder taper", detail: "14.20 mm cushion face · 66% taper. A continuous, solid shoulder returns to the palm.", layers: &[]},
    Step {title: "Build the drafted seal", tool: "Profile border, curve remap & window", detail: "A rounded axial cushion supports the ornament. A monotone curve softens its crest while preserving the withdrawal slope.", layers: &["Axial cushion / positive draft"]},
    Step {title: "Frame the composition", tool: "Vector alpha stamp", detail: "One broad, softly bevelled octagonal line. Centred on the head, with a clean margin around the seal.", layers: &["Broad octagonal seal frame"]},
    Step {title: "Place the eight-petal flower", tool: "Original SVG relief", detail: "Eight broad, sculpted petals meet at the centre. Their 0.10 mm relief gives the seal a shallow, continuous impression.", layers: &["Eight-petal Aster seal"]},
    Step {title: "Burnish the flower heart", tool: "Solid metal boss", detail: "A centred 3.20 mm rounded heart. Solid metal, with no stone pocket or projecting prongs.", layers: &["Burnished flower heart"]},
    Step {title: "Add paired palmettes", tool: "Mirrored alpha stamps", detail: "Botanical cheek seals on both annular faces. The relief faces the opposed mould pulls.", layers: &["Paired palmette cheek seals"]},
    Step {title: "Trace the polished rails", tool: "Mirrored profile borders", detail: "Two rounded rails frame each cheek. Their spacing leaves the ornament room to read.", layers: &["Inner polished cheek rail", "Outer polished cheek rail"]},
    Step {title: "Weave the shoulder ground", tool: "Tiling, mask, warp & remap", detail: "Sunseed texture follows a curved guide, constrained to the shoulders by an original mask.", layers: &["Warped sunseed tessellation / shoulder mask"]},
    Step {title: "Lay the flowing vines", tool: "Swept curve wires", detail: "Mirrored vines use a smooth union and fade before the seal. Rounded 0.48 mm wires remain shallow.", layers: &["Woven vine shoulder wires"]},
    Step {title: "Set the pearl edging", tool: "Milgrain", detail: "Seventy-six rounded beads at 0.56 mm diameter. A window keeps the head's margin uncluttered.", layers: &["Pearl edging"]},
    Step {title: "Fan the shoulders", tool: "Axial flutes & soft window", detail: "Broad 1.05 mm reeds follow the pull direction. Their 0.12 mm relief catches the light without deep cuts.", layers: &["Broad axial shoulder fans"]},
    Step {title: "Finish the palm rhythm", tool: "Windowed reeding", detail: "A finer 0.70 mm reed cadence closes the underside. The finger opening stays smooth and clear.", layers: &["Axial palm reeds"]},
    Step {title: "Prepare the sand pattern", tool: "Manufacturing recipe", detail: "14k gold · Delft clay · Z=0 parting · opposed Z pull. Inspect the compensated mesh before exporting.", layers: &[]},
];

pub fn source() -> &'static RingDesign {
    static SOURCE: OnceLock<RingDesign> = OnceLock::new();
    SOURCE.get_or_init(|| crate::library::load_design_str(include_str!("../assets/aster-atelier.ring.json")).expect("bundled Aster Atelier source"))
}

pub fn blank() -> RingDesign {
    RingDesign { name: source().name.clone(), build: source().build, ..Default::default() }
}

fn same<T: serde::Serialize>(a: &T, b: &T) -> bool {
    serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
}

fn merge<T: Clone>(dest: &mut Vec<T>, src: &[T], name: impl Fn(&T) -> &str) {
    for item in src {
        if let Some(existing) = dest.iter_mut().find(|e| name(e) == name(item)) { *existing=item.clone(); }
        else { dest.push(item.clone()); }
    }
}

pub fn present(d: &RingDesign, step: usize) -> bool {
    match step {
        0 => same(&d.profile, &source().profile) && d.size == source().size,
        1 => same(&d.shank, &source().shank),
        13 => same(&d.manufacturing, &source().manufacturing),
        _ => STEPS.get(step).is_some_and(|s| s.layers.iter().all(|name| d.layers.layers.iter().any(|e| e.name == *name))),
    }
}

/// Insert the operation's editable layers in their compositing order. Reapplying
/// updates those layers in place, without duplicating or discarding other work.
pub fn apply(d: &mut RingDesign, step: usize, strength: f64) -> anyhow::Result<()> {
    anyhow::ensure!(step < STEPS.len(), "Unknown construction step");
    anyhow::ensure!(strength.is_finite() && (0.1..=1.0).contains(&strength), "Relief strength must be 10–100%");
    let target = source();
    match step {
        0 => { d.profile = target.profile.clone(); d.size = target.size; }
        1 => { d.shank = target.shank.clone(); }
        13 => { d.manufacturing = target.manufacturing.clone(); d.draft = target.draft; }
        _ => {
            anyhow::ensure!(present(d, 0) && present(d, 1), "Form the band and signet head first");
            merge(&mut d.embedded,&target.embedded,|a| &a.name);
            merge(&mut d.svgs,&target.svgs,|a| &a.name);
            merge(&mut d.recipes,&target.recipes,|a| &a.name);
            merge(&mut d.drawn,&target.drawn,|a| &a.name);
            merge(&mut d.texts,&target.texts,|a| &a.name);
            let rank = |name: &str| target.layers.layers.iter().position(|e| e.name == name);
            for name in STEPS[step].layers {
                let at = rank(name).ok_or_else(|| anyhow::anyhow!("Missing recipe layer {name}"))?;
                let mut entry: LayerEntry = target.layers.layers[at].clone();
                entry.opacity *= strength;
                if let Some(existing) = d.layers.layers.iter().position(|e| e.name == *name) {
                    d.layers.layers[existing] = entry;
                } else {
                    let pos = d.layers.layers.iter().position(|e| rank(&e.name).is_some_and(|r| r > at)).unwrap_or(d.layers.layers.len());
                    d.layers.layers.insert(pos, entry);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn steps_reproduce_source_and_reapply_without_duplicates() {
        let mut d = blank();
        assert!(d.layers.layers.is_empty());
        assert!(apply(&mut d, 4, 1.).is_err());
        for i in 0..STEPS.len() { apply(&mut d, i, 1.).unwrap(); assert!(present(&d, i)); }
        assert!(same(&d, source()), "guide must reproduce the reviewed source");
        apply(&mut d, 4, 0.5).unwrap();
        assert_eq!(d.layers.layers.len(), source().layers.layers.len());
        apply(&mut d, 4, 1.).unwrap();
        assert!(same(&d, source()));
        assert!(apply(&mut d, 4, f64::NAN).is_err());
        d.drawn.push(crate::drawn::DrawnAlpha::new("User sketch",32,32));
        d.layers.layers.push(LayerEntry::new("User border",crate::field::Layer::Border(Default::default())));
        apply(&mut d,4,1.).unwrap();
        assert!(d.drawn.iter().any(|a| a.name=="User sketch"));
        assert!(d.layers.layers.iter().any(|e| e.name=="User border"));
    }
}
