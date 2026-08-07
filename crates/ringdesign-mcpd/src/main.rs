//! ringdesign-mcpd — headless RingDesigner MCP server.
//!
//! ```text
//! ringdesign-mcpd                  stdio MCP server (default)
//! ringdesign-mcpd --http [PORT]    streamable-HTTP MCP server (default 8732)
//! ringdesign-mcpd --demo [OUT]     build the default ring, print a report, write an STL
//! ringdesign-mcpd --help
//! ```
//!
//! The alpha library is loaded from the standard user directories, so the
//! daemon and the GUI see the same patterns.
//!
//! # Claude Desktop / Claude Code config
//!
//! stdio — `claude_desktop_config.json`, or `.mcp.json` in a project:
//!
//! ```json
//! {
//!   "mcpServers": {
//!     "ringdesign": {
//!       "command": "/home/shadowbroker/Documents/Rust/JewelryProjects/RingDesigner/target/debug/ringdesign-mcpd",
//!       "args": []
//!     }
//!   }
//! }
//! ```
//!
//! streamable HTTP — start `ringdesign-mcpd --http 8732` first, then:
//!
//! ```json
//! {
//!   "mcpServers": {
//!     "ringdesign": {
//!       "type": "http",
//!       "url": "http://127.0.0.1:8732/"
//!     }
//!   }
//! }
//! ```
//!
//! Use the HTTP form to attach to a running GUI, which serves the same engine
//! and repaints when an agent edits the design.

use std::net::SocketAddr;
use std::sync::Arc;

use ringdesign_core::castability::CastReport;
use ringdesign_core::mesh::Report;
use ringdesign_core::{DesignEngine, SharedEngine};
use ringdesign_mcp::{DEFAULT_PORT, NotifyFn, serve_http, serve_stdio};

const USAGE: &str = "\
ringdesign-mcpd — MCP server for the RingDesigner sand-casting geometry engine

USAGE:
    ringdesign-mcpd                  serve MCP over stdio (default)
    ringdesign-mcpd --http [PORT]    serve streamable HTTP on 127.0.0.1:PORT
    ringdesign-mcpd --demo [OUT]     build the default ring, print a report, write an STL
    ringdesign-mcpd --help           this text

ENVIRONMENT:
    RUST_LOG    log filter, e.g. RUST_LOG=debug. All logging goes to stderr.
";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging();

    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--demo") => demo(args.get(1).map(String::as_str)),
        Some("--http") => http(args.get(1).and_then(|s| s.parse().ok())).await,
        Some("--help" | "-h") => {
            eprint!("{USAGE}");
            Ok(())
        }
        Some(other) => {
            eprintln!("unknown argument: {other}\n");
            eprint!("{USAGE}");
            std::process::exit(2);
        }
        None => serve_stdio(engine()).await,
    }
}

/// Logs go to stderr because the stdio transport uses stdout as the MCP frame
/// channel; anything else on stdout desynchronises the client.
fn init_logging() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Stderr)
        .init();
}

fn engine() -> SharedEngine {
    DesignEngine::shared_with_disk_library()
}

async fn http(port: Option<u16>) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port.unwrap_or(DEFAULT_PORT)));
    let notify: NotifyFn = Arc::new(|| {});
    serve_http(engine(), addr, notify).await
}

/// The only subcommand that writes to stdout.
fn demo(out: Option<&str>) -> anyhow::Result<()> {
    let path = out.unwrap_or("/tmp/ringdesign-demo.stl");
    let engine = engine();
    let mut e = engine.lock();

    let report = e.build(None);
    let cast = e.castability();
    let bytes = e.export_stl(path)?;

    println!("== RingDesigner demo ==");
    println!(
        "design: {}   size {}   {} band {:.2} x {:.2} mm",
        e.design().name,
        e.design().size.display(),
        e.design().profile.style.label(),
        e.design().profile.width_mm,
        e.design().profile.thickness_mm,
    );
    print_report(&report);
    print_castability(&cast);
    println!("\nSTL: {path} ({bytes} bytes)");
    Ok(())
}

fn print_report(r: &Report) {
    let v = &r.validation;
    println!(
        "\nwatertight: {}  triangles: {}  vertices: {}  boundary_edges: {}  non_manifold_edges: {}",
        v.watertight, v.triangle_count, v.vertex_count, v.boundary_edges, v.non_manifold_edges
    );
    println!(
        "bounds: {:.2} x {:.2} x {:.2} mm   volume: {:.1} mm^3   area: {:.1} mm^2",
        r.bounds_mm[0], r.bounds_mm[1], r.bounds_mm[2], r.volume_mm3, r.surface_area_mm2
    );
    println!(
        "inner dia: {:.2} mm   outer dia: {:.2} mm   band width: {:.2} mm",
        r.inner_diameter_mm, r.outer_diameter_mm, r.band_width_mm
    );
    println!(
        "relief: +{:.3} mm raised / {:.3} mm engraved   built in {} ms",
        r.max_relief_mm, r.min_relief_mm, r.build_ms
    );

    println!("\n-- Metal weights --");
    for m in &r.metals {
        println!("  {:<16} {:>7.2} g   {:>7.2} dwt", m.metal, m.grams, m.dwt);
    }
}

fn print_castability(c: &CastReport) {
    let pct = c.undercut_fraction() * 100.0;
    println!("\n-- Sand castability (mould parts at z = {:.2} mm, pulls +/-Z) --", c.parting_z_mm);
    println!("verdict: {}", c.verdict.label());
    println!(
        "faces: {} good  {} marginal  {} vertical  {} undercut",
        c.good, c.marginal, c.vertical, c.undercut
    );
    println!(
        "undercut area: {:.2} mm^2 ({pct:.2}% of {:.1})   marginal: {:.2} mm^2   worst draft: {:.1} deg",
        c.undercut_area_mm2, c.total_area_mm2, c.marginal_area_mm2, c.worst_draft_deg
    );
    for note in &c.notes {
        println!("  - {note}");
    }
}
