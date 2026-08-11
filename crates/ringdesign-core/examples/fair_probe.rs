//! Prints the heart outline's fairing metrics at the current `BODY_FAIR_R`.

use ringdesign_core::field::SignetOutline;

fn main() {
    let o = SignetOutline::Heart;
    let notch = |e: &dyn Fn(f64) -> (f64, f64)| {
        e(0.0).0 - (0..=400).map(|i| e(i as f64 / 400.0).0).fold(0.0f64, f64::min)
    };
    let face = notch(&|x| o.extent(x));
    let body = notch(&|x| o.body_extent(x));
    let (lo, hi) = o.body_extent(1.0);
    println!("face_notch {face:.4}  body_notch {body:.4}  end_width {:.4}", hi - lo);
}
