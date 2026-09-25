use crate::{
    RingDesign,
    stones::{self, StoneFrame},
};

/// Display envelopes follow the same superellipse approximation as the stone
/// census. They are not exact collision solids for every fancy-cut pavilion.
pub struct Envelope {
    pub label: String,
    pub centre: [f64; 3],
    pub girdle: Vec<[f64; 3]>,
    pub deep: Vec<[f64; 3]>,
    pub worst_gap: f64,
}
pub fn envelopes(d: &RingDesign, gap: f64) -> Vec<Envelope> {
    let frames = stones::all_stone_frames(d);
    frames
        .iter()
        .enumerate()
        .map(|(i, (st, f))| {
            let ring = |depth: f64| outline(&f, gap.max(0.0) * 0.5, depth);
            let worst_gap = frames
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, (_, other))| {
                    let [a, b] = f.clearance_to(other);
                    a.min(b)
                })
                .fold(f64::INFINITY, f64::min);
            Envelope {
                label: st.label.clone(),
                centre: f.girdle,
                girdle: ring(0.0),
                deep: ring(f.pavilion),
                worst_gap,
            }
        })
        .collect()
}
fn outline(f: &StoneFrame, margin: f64, depth: f64) -> Vec<[f64; 3]> {
    (0..48)
        .map(|i| {
            let angle = std::f64::consts::TAU * i as f64 / 48.0;
            let (s, c) = angle.sin_cos();
            let r =
                crate::field::superellipse_radius_mm(c, s, f.semi.0, f.semi.1, f.plan_pow) + margin;
            std::array::from_fn(|j| {
                f.girdle[j] + r * (c * f.long[j] + s * f.short[j]) - depth * f.normal[j]
            })
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn margin_is_shared_between_neighbours_and_pavilion_follows_normal() {
        let f = StoneFrame {
            girdle: [0.0; 3],
            normal: [0.0, 0.0, 1.0],
            long: [1.0, 0.0, 0.0],
            short: [0.0, 1.0, 0.0],
            semi: (2.0, 1.0),
            plan_pow: 2.0,
            reach: 2.0,
            pavilion: 1.5,
        };
        let p = outline(&f, 0.2, 1.5);
        assert!((p[0][0] - 2.2).abs() < 1e-8);
        assert_eq!(p[0][2], -1.5);
    }
}
