//! MCP server exposing the ring designer: tools, resources, and prompts over
//! stdio or streamable HTTP.
//!
//! Everything operates on a shared [`DesignEngine`], so a GUI hosting this
//! server sees agent edits and vice versa — the engine's generation counter is
//! what each side polls to notice the other's changes.

use std::net::SocketAddr;
use std::sync::Arc;

use ringdesign_core::SharedEngine;
use rmcp::ServiceExt;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;

pub mod prompts;
pub mod resources;
pub mod sync;
pub mod tools;

/// Called after any mutation so a host can repaint.
pub type NotifyFn = Arc<dyn Fn() + Send + Sync>;

/// Default port for the streamable-HTTP transport.
pub const DEFAULT_PORT: u16 = 8732;

#[derive(Clone)]
pub struct RingDesignServer {
    pub(crate) engine: SharedEngine,
    pub(crate) notify: Option<NotifyFn>,
}

impl RingDesignServer {
    pub fn new(engine: SharedEngine) -> Self {
        Self { engine, notify: None }
    }

    pub fn with_notify(engine: SharedEngine, notify: NotifyFn) -> Self {
        Self { engine, notify: Some(notify) }
    }

    pub(crate) fn touch(&self) {
        if let Some(n) = &self.notify {
            n();
        }
    }
}

/// Serve over stdio, blocking until the client disconnects.
pub async fn serve_stdio(engine: SharedEngine) -> anyhow::Result<()> {
    let service = RingDesignServer::new(engine)
        .serve(rmcp::transport::io::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}

/// A streamable-HTTP service sharing `engine`.
pub fn http_service(
    engine: SharedEngine,
    notify: NotifyFn,
) -> StreamableHttpService<RingDesignServer, LocalSessionManager> {
    StreamableHttpService::new(
        move || Ok(RingDesignServer::with_notify(engine.clone(), notify.clone())),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    )
}

/// Bind and serve streamable HTTP until the task is dropped.
pub async fn serve_http(
    engine: SharedEngine,
    addr: SocketAddr,
    notify: NotifyFn,
) -> anyhow::Result<()> {
    let service = http_service(engine, notify);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    log::info!("ringdesign MCP (streamable-http) on http://{addr}/");
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let io = hyper_util::rt::TokioIo::new(stream);
        let svc = hyper_util::service::TowerToHyperService::new(service.clone());
        tokio::spawn(async move {
            let _ = hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new())
                .serve_connection(io, svc)
                .await;
        });
    }
}
