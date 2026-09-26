28. **A faired bypass arm union** (opt-in, fenced at format 6 when non-zero): `ShankStyle::bypass_fair_deg`
    (serde default 0 keeps the hard union), and `bypass_span_faired(off, k, fair_deg)` averaging
    `bypass_span` over +-fair_deg with weights `(1 - t^2)^2` (N = 12 each side), called in
    `modulation()`'s `ShankKind::Bypass` arm. About 4 deg turns the arm tip's re-entrant corner into a
    ramp; Corvus's skulls fold about 0.6 mm sideways where the bare normal turns 12 deg within 2 deg of
    ring angle under 2.9 mm of relief.

The requesting author's proposed code:
```rust
// profile.rs, ShankStyle (fence at format 6 when non-zero via library::format_version_for)
/// Degrees along the ring a bypass's arm union is faired over; 0 keeps the hard union.
#[serde(default)]
pub bypass_fair_deg: f64,

pub fn bypass_span_faired(off: f64, k: f64, fair_deg: f64) -> (f64, f64) {
    if fair_deg <= 0.0 {
        return bypass_span(off, k);
    }
    const N: i32 = 12;
    let (mut lo, mut hi, mut sum) = (0.0, 0.0, 0.0);
    for i in -N..=N {
        let t = i as f64 / N as f64;
        let w = (1.0 - t * t).powi(2);
        let (l, h) = bypass_span(off + t * fair_deg, k);
        lo += w * l;
        hi += w * h;
        sum += w;
    }
    (lo / sum, hi / sum)
}
// in modulation()'s ShankKind::Bypass arm: let (lo, hi) = bypass_span_faired(off, k, self.bypass_fair_deg);
```
Also expose it where the other shank parameters are exposed (the GUI shank panel if it lists bypass settings, the MCP shank tool, the graph shank node's struct coverage) so a ring author can set it, and check the field verdict stays clean on a bypass with it at 4 degrees.