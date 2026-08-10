//! Round-trip the sync endpoint against a real listener: pull a design, change it, push it back,
//! and confirm the engine the desktop reads from actually moved.

use ringdesign_core::{DesignEngine, RingDesign};
use ringdesign_mcp::sync;

fn main() -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?;

    let engine = DesignEngine::shared(ringdesign_core::alpha::AlphaLibrary::builtin());
    {
        let mut e = engine.lock();
        let mut d = RingDesign::default();
        d.name = "from desktop".into();
        e.set_design(d);
    }

    // Port 0 would be invisible to the client, so pick a high fixed one.
    let port = 8899u16;
    let cfg = sync::Config { addr: std::net::SocketAddr::from(([127, 0, 0, 1], port)), token: Some("test-token-1234".into()) };
    let served = engine.clone();
    rt.spawn(async move { sync::serve(served, cfg, None).await });
    std::thread::sleep(std::time::Duration::from_millis(300));

    let base = format!("http://127.0.0.1:{port}");
    let hdr = ("x-ring-token", "test-token-1234");

    // No token at all must be refused.
    let unauth = minreq::get(format!("{base}/design")).send()?;
    assert_eq!(unauth.status_code, 401, "an untokened read got through");

    let health = minreq::get(format!("{base}/health")).with_header(hdr.0, hdr.1).send()?;
    assert_eq!(health.status_code, 200);
    println!("health  : {}", health.as_str()?);

    let got = minreq::get(format!("{base}/design")).with_header(hdr.0, hdr.1).send()?;
    assert_eq!(got.status_code, 200);
    let mut design: RingDesign = serde_json::from_str(got.as_str()?)?;
    println!("pulled  : {:?} size {:?}", design.name, design.size);
    assert_eq!(design.name, "from desktop");

    design.name = "from phone".into();
    design.size = ringdesign_core::RingSize(9.5);
    let body = serde_json::to_vec(&design)?;
    let put = minreq::post(format!("{base}/design"))
        .with_header(hdr.0, hdr.1)
        .with_body(body)
        .send()?;
    assert_eq!(put.status_code, 200, "push rejected: {}", put.as_str()?);
    println!("pushed  : {}", put.as_str()?);

    let after = engine.lock().design().clone();
    assert_eq!(after.name, "from phone", "the engine did not take the push");
    assert_eq!(after.size.0, 9.5);
    println!("engine  : {:?} size {:?}", after.name, after.size);

    let junk = minreq::post(format!("{base}/design"))
        .with_header(hdr.0, hdr.1)
        .with_body("not a design")
        .send()?;
    assert_eq!(junk.status_code, 400);
    println!("junk    : {} (rejected)", junk.status_code);

    println!("\nsync round-trip OK");
    Ok(())
}
