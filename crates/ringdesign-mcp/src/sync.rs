//! A plain-HTTP sync endpoint for the phone, sharing the same [`SharedEngine`] as the MCP server.
//!
//! Why not MCP: the phone would have to speak streamable-HTTP JSON-RPC with session management to
//! read one struct, and the MCP tools cannot do the job anyway — `save_design`/`load_design` take
//! paths on the *server's* filesystem, so `load_design` called from a phone would load a file on
//! the desktop. There is no push. Two REST routes and a token is the whole requirement.
//!
//! Both sides see the same `DesignEngine`, so a pull reflects whatever the desktop is showing and a
//! push lands in the running app, which notices via the generation counter and repaints.
//!
//! # Binding
//!
//! Loopback is the default and stays the default. `0.0.0.0` on an untrusted network would be an
//! unauthenticated tool for rewriting someone's live design, so callers opt in explicitly and the
//! token is not optional when they do — [`Config::remote`] refuses to build without one.

use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::{Method, Request, Response, StatusCode};
use ringdesign_core::{RingDesign, SharedEngine};

use crate::NotifyFn;

/// Header carrying the shared secret.
pub const TOKEN_HEADER: &str = "x-ring-token";
/// Default port: one past the MCP server's, so both can run side by side.
pub const DEFAULT_SYNC_PORT: u16 = crate::DEFAULT_PORT + 1;
/// Largest design accepted, so a stray POST cannot exhaust memory. A design carrying a few thousand
/// pen strokes is a few hundred KB; this is generous.
const MAX_BODY: usize = 8 * 1024 * 1024;

/// Where and how to serve.
#[derive(Clone, Debug)]
pub struct Config {
    pub addr: SocketAddr,
    /// Required for anything not on loopback.
    pub token: Option<String>,
}

impl Config {
    /// Loopback only. No token needed: nothing off-machine can reach it.
    pub fn local(port: u16) -> Self {
        Self { addr: SocketAddr::from(([127, 0, 0, 1], port)), token: None }
    }

    /// Bind `ip` — meant for a tailnet address — with a required token.
    pub fn remote(ip: IpAddr, port: u16, token: impl Into<String>) -> anyhow::Result<Self> {
        let token = token.into();
        if token.len() < 8 {
            anyhow::bail!("a remote sync token must be at least 8 characters");
        }
        Ok(Self { addr: SocketAddr::new(ip, port), token: Some(token) })
    }

    fn authorized(&self, req: &Request<hyper::body::Incoming>) -> bool {
        match self.token.as_deref() {
            None => true,
            Some(want) => req
                .headers()
                .get(TOKEN_HEADER)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|got| constant_time_eq(got, want)),
        }
    }
}

/// Compare without leaking length-prefix timing. Short strings, but free to do properly.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The tailnet address of this machine, if it is on one.
///
/// Tailscale hands every node an address in `100.64.0.0/10` (the CGNAT range it borrows), so
/// finding one is enough to bind somewhere only the tailnet can reach — no shelling out to
/// `tailscale ip`, and no exposure to the local coffee-shop LAN.
pub fn tailnet_addr() -> Option<IpAddr> {
    let out = std::process::Command::new("ip")
        .args(["-4", "-o", "addr", "show"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let Some(rest) = line.split("inet ").nth(1) else { continue };
        let Some(ip_str) = rest.split('/').next() else { continue };
        let Ok(ip) = ip_str.trim().parse::<std::net::Ipv4Addr>() else { continue };
        let o = ip.octets();
        // 100.64.0.0/10 is 100.64.x.x through 100.127.x.x.
        if o[0] == 100 && (64..=127).contains(&o[1]) {
            return Some(IpAddr::V4(ip));
        }
    }
    None
}

fn json(status: StatusCode, body: String) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(body)))
        .expect("static response builds")
}

async fn route(
    req: Request<hyper::body::Incoming>,
    engine: SharedEngine,
    cfg: Arc<Config>,
    notify: Option<NotifyFn>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    if !cfg.authorized(&req) {
        return Ok(json(StatusCode::UNAUTHORIZED, r#"{"ok":false,"error":"bad token"}"#.into()));
    }

    let resp = match (&method, path.as_str()) {
        (&Method::GET, "/health") => {
            let g = engine.lock().generation();
            json(StatusCode::OK, format!(r#"{{"ok":true,"app":"ringdesigner","generation":{g}}}"#))
        }
        (&Method::GET, "/design") => {
            let design = engine.lock().design().clone();
            match serde_json::to_string(&design) {
                Ok(body) => json(StatusCode::OK, body),
                Err(e) => json(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!(r#"{{"ok":false,"error":"{e}"}}"#),
                ),
            }
        }
        (&Method::POST, "/design") => {
            let collected = req.into_body().collect().await;
            let Ok(body) = collected else {
                return Ok(json(
                    StatusCode::BAD_REQUEST,
                    r#"{"ok":false,"error":"unreadable body"}"#.into(),
                ));
            };
            let bytes = body.to_bytes();
            if bytes.len() > MAX_BODY {
                return Ok(json(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    r#"{"ok":false,"error":"design too large"}"#.into(),
                ));
            }
            match serde_json::from_slice::<RingDesign>(&bytes) {
                Ok(design) => {
                    let g = {
                        let mut e = engine.lock();
                        e.set_design(design);
                        e.generation()
                    };
                    if let Some(n) = notify.as_ref() {
                        n();
                    }
                    json(StatusCode::OK, format!(r#"{{"ok":true,"generation":{g}}}"#))
                }
                Err(e) => json(
                    StatusCode::BAD_REQUEST,
                    format!(r#"{{"ok":false,"error":"not a design: {e}"}}"#),
                ),
            }
        }
        _ => json(StatusCode::NOT_FOUND, r#"{"ok":false,"error":"no such route"}"#.into()),
    };
    Ok(resp)
}

/// Bind and serve until the task is dropped.
pub async fn serve(
    engine: SharedEngine,
    cfg: Config,
    notify: Option<NotifyFn>,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(cfg.addr).await?;
    log::info!(
        "ringdesign sync on http://{}/ ({})",
        cfg.addr,
        if cfg.token.is_some() { "token required" } else { "loopback, no token" }
    );
    let cfg = Arc::new(cfg);
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue };
        let io = hyper_util::rt::TokioIo::new(stream);
        let (engine, cfg, notify) = (engine.clone(), cfg.clone(), notify.clone());
        tokio::spawn(async move {
            let svc = hyper::service::service_fn(move |req| {
                route(req, engine.clone(), cfg.clone(), notify.clone())
            });
            let _ = hyper_util::server::conn::auto::Builder::new(
                hyper_util::rt::TokioExecutor::new(),
            )
            .serve_connection(io, svc)
            .await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_local_config_needs_no_token_and_stays_on_loopback() {
        let c = Config::local(1234);
        assert!(c.token.is_none());
        assert!(c.addr.ip().is_loopback());
    }

    #[test]
    fn a_remote_config_refuses_a_weak_token() {
        let ip: IpAddr = "100.101.102.103".parse().unwrap();
        assert!(Config::remote(ip, 1, "short").is_err());
        assert!(Config::remote(ip, 1, "long-enough-token").is_ok());
    }

    #[test]
    fn constant_time_eq_matches_string_equality() {
        assert!(constant_time_eq("abcd", "abcd"));
        assert!(!constant_time_eq("abcd", "abce"));
        assert!(!constant_time_eq("abcd", "abcde"));
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn the_sync_port_does_not_collide_with_the_mcp_one() {
        assert_ne!(DEFAULT_SYNC_PORT, crate::DEFAULT_PORT);
    }

    #[test]
    fn tailnet_detection_only_ever_returns_a_cgnat_address() {
        // Whether this machine is on a tailnet is not something a test can assume; what it can
        // assert is that a positive answer is always in the range Tailscale actually uses.
        if let Some(IpAddr::V4(ip)) = tailnet_addr() {
            let o = ip.octets();
            assert_eq!(o[0], 100);
            assert!((64..=127).contains(&o[1]), "{ip} is not in 100.64.0.0/10");
        }
    }
}
