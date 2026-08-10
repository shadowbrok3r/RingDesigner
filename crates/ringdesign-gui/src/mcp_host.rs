//! Hosts the MCP server inside the running app.
//!
//! The app keeps owning its `RingDesign`; a [`SharedEngine`] runs alongside on
//! its own tokio runtime and the two are reconciled every frame through the
//! engine's generation counter — [`McpHost::poll`] pulls agent edits in,
//! [`McpHost::push`] sends local edits out.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use ringdesign_core::{DesignEngine, RingDesign, SharedEngine};

/// A streamable-HTTP MCP server serving the app's live design.
pub struct McpHost {
    engine: SharedEngine,
    addr: SocketAddr,
    /// Engine generation this app has already absorbed.
    last_seen: u64,
    /// The design behind `last_seen`, serialized.
    last_json: Vec<u8>,
    /// Dropping this stops the listener.
    runtime: tokio::runtime::Runtime,
    /// Where the phone-facing sync endpoint is bound, once started.
    sync_addr: Option<SocketAddr>,
}

impl McpHost {
    /// Bind `127.0.0.1:port` and serve `design`. A taken port returns an error
    /// rather than panicking; port 0 resolves to a free one.
    pub fn start(design: &RingDesign, port: u16, ctx: egui::Context) -> anyhow::Result<Self> {
        // Probe bind resolves the port and reports one already in use.
        let probe = std::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))?;
        let addr = probe.local_addr()?;
        drop(probe);

        let engine = DesignEngine::shared_with_disk_library();
        let last_seen = {
            let mut e = engine.lock();
            e.set_design(design.clone());
            e.generation()
        };
        let last_json = snapshot(design);

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("ringdesign-mcp")
            .enable_all()
            .build()?;

        // Repaints the viewport on every agent mutation.
        let notify: ringdesign_mcp::NotifyFn = Arc::new(move || ctx.request_repaint());
        let served = engine.clone();
        runtime.spawn(async move {
            if let Err(e) = ringdesign_mcp::serve_http(served, addr, notify).await {
                log::error!("MCP server on {addr} stopped: {e}");
            }
        });

        log::info!("MCP server listening on http://{addr}/");
        Ok(Self { engine, addr, last_seen, last_json, runtime, sync_addr: None })
    }

    /// Also serve the plain-HTTP sync endpoint the phone talks to.
    ///
    /// Loopback unless a tailnet address is found, and never off-loopback without a token: an open
    /// port here lets anyone who can reach it replace the design you are looking at. Tailscale is
    /// the right boundary — the tailnet is authenticated, the coffee-shop LAN is not — so this
    /// binds the `100.64.0.0/10` address specifically rather than `0.0.0.0`.
    pub fn start_sync(&mut self, token: &str, remote: bool) -> anyhow::Result<SocketAddr> {
        let cfg = if remote {
            let ip = ringdesign_mcp::sync::tailnet_addr()
                .ok_or_else(|| anyhow::anyhow!("no tailnet address — is Tailscale up?"))?;
            ringdesign_mcp::sync::Config::remote(ip, ringdesign_mcp::sync::DEFAULT_SYNC_PORT, token)?
        } else {
            ringdesign_mcp::sync::Config::local(ringdesign_mcp::sync::DEFAULT_SYNC_PORT)
        };
        let addr = cfg.addr;
        // Probe first so a taken port is an error the UI can show, not a task that dies silently.
        drop(std::net::TcpListener::bind(addr)?);

        let served = self.engine.clone();
        self.runtime.spawn(async move {
            if let Err(e) = ringdesign_mcp::sync::serve(served, cfg, None).await {
                log::error!("sync server stopped: {e}");
            }
        });
        self.sync_addr = Some(addr);
        Ok(addr)
    }

    pub fn sync_addr(&self) -> Option<SocketAddr> {
        self.sync_addr
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Pull agent edits in. Returns true when the design changed underneath us.
    ///
    /// The generation also advances on a build, which an agent triggers by
    /// merely reading a report, so the design itself is compared before the app
    /// is told anything changed.
    pub fn poll(&mut self, design: &mut RingDesign) -> bool {
        let engine = self.engine.lock();
        if engine.generation() == self.last_seen {
            return false;
        }
        self.last_seen = engine.generation();
        let json = snapshot(engine.design());
        if json == self.last_json {
            return false;
        }
        *design = engine.design().clone();
        self.last_json = json;
        true
    }

    /// Push a local edit out to the engine without looking like an agent edit.
    pub fn push(&mut self, design: &RingDesign) {
        let mut engine = self.engine.lock();
        engine.set_design(design.clone());
        self.last_seen = engine.generation();
        self.last_json = snapshot(design);
    }
}

/// Serialized design, empty when it will not serialize so the next poll differs.
fn snapshot(design: &RingDesign) -> Vec<u8> {
    serde_json::to_vec(design).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_sees_engine_edits_and_push_does_not_echo() {
        let mut design = RingDesign::default();
        let mut host =
            McpHost::start(&design, 0, egui::Context::default()).expect("bind an ephemeral port");
        assert!(!host.poll(&mut design), "a fresh host reported an edit");

        host.engine.lock().design_mut().name = "Agent".into();
        assert!(host.poll(&mut design), "an engine edit went unnoticed");
        assert_eq!(design.name, "Agent");
        assert!(!host.poll(&mut design), "the same edit polled twice");

        design.name = "Local".into();
        host.push(&design);
        assert_eq!(host.engine.lock().design().name, "Local");
        assert!(!host.poll(&mut design), "a local push read back as an agent edit");
    }

    #[test]
    fn an_agent_build_bumps_the_generation_without_reporting_an_edit() {
        let mut design = RingDesign::default();
        let mut host =
            McpHost::start(&design, 0, egui::Context::default()).expect("bind an ephemeral port");
        {
            let mut e = host.engine.lock();
            e.design_mut().build = ringdesign_core::BuildParams {
                theta_steps: 48,
                profile_steps: 32,
                ..Default::default()
            };
        }
        assert!(host.poll(&mut design), "the resolution change went unnoticed");

        let g = host.engine.lock().generation();
        host.engine.lock().build(None);
        assert!(host.engine.lock().generation() > g, "build did not advance the generation");
        assert!(!host.poll(&mut design), "a read-only agent build read back as a design edit");
    }

    #[test]
    fn a_taken_port_errors_instead_of_panicking() {
        let taken = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = taken.local_addr().unwrap().port();
        assert!(McpHost::start(&RingDesign::default(), port, egui::Context::default()).is_err());
    }
}
