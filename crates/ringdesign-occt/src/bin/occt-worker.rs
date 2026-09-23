//! One OpenCascade request on stdin, one response on stdout.
fn main() {
    std::process::exit(ringdesign_occt::kernel::serve());
}
