//! Windows executable resources: the icon, the version block and the manifest.
//!
//! Nothing here runs for a Linux target — there a window manager takes its icon
//! from the running process ([`ringdesign_assets::app_icon_rgba`], set in `main`)
//! and from the `.desktop` file an install drops beside it. Windows wants the
//! icon inside the `.exe` as well, or Explorer and the taskbar show the
//! generic one.
//!
//! The `.ico` is cut from the same SVG as the window icon, by
//! `ringdesign-assets`' build script, and reaches here through that crate's
//! `links` metadata — so the two can never drift.
//!
//! The target is read from `CARGO_CFG_TARGET_OS`, never from `cfg!(windows)`:
//! a build script is compiled for the host, so that attribute is false in a
//! Linux-to-Windows cross-build and the executable came out bare.

use std::collections::HashMap;
use std::env;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    texture_gate();
    occt_worker();
    commit();
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        resources();
    }
}

/// The workspace root: the folder holding the root `Cargo.toml`, two above
/// this crate's.
fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    manifest.parent().and_then(Path::parent).map_or(manifest.clone(), Path::to_path_buf)
}

/// The commit the app is built from, as `RINGDESIGNER_COMMIT`: that variable
/// when set (the portable container has no repository to ask), else CI's
/// `GITHUB_SHA`, else `git rev-parse HEAD`, else `unknown`. The Licences
/// window shows it beside the repository's address.
fn commit() {
    println!("cargo:rerun-if-env-changed=RINGDESIGNER_COMMIT");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(workspace_root())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let named = ["RINGDESIGNER_COMMIT", "GITHUB_SHA"].into_iter().find_map(|v| env::var(v).ok().filter(|s| !s.trim().is_empty()));
    let commit = named.or_else(|| {
        // Watched so a new commit or checkout rebuilds with its name; a path that is not there is not watched.
        let mut watched = vec!["HEAD".to_string(), "packed-refs".to_string()];
        watched.extend(git(&["symbolic-ref", "-q", "HEAD"]));
        for name in watched {
            if let Some(path) = git(&["rev-parse", "--path-format=absolute", "--git-path", &name]).map(PathBuf::from).filter(|p| p.is_file()) {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
        git(&["rev-parse", "HEAD"])
    });
    let commit = commit.filter(|c| !c.contains(['\n', '\r'])).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=RINGDESIGNER_COMMIT={commit}");
}

/// The OpenCascade worker the app carries, when `RINGDESIGNER_OCCT_WORKER`
/// names a built one; a relative path is read from the workspace root.
///
/// The file is deflated into `OUT_DIR` and described, with its SHA-256 and
/// length, by a generated `occt_worker.rs` that `src/occt_embedded.rs`
/// includes; the app unpacks it into its data folder on first use. Without the
/// variable the generated file names no worker and nothing is embedded, so an
/// ordinary build and every CI job are unchanged. The file's first bytes are
/// checked against the target, so a Linux worker cannot ride inside a Windows
/// app.
fn occt_worker() {
    use flate2::{Compression, write::DeflateEncoder};
    use sha2::{Digest, Sha256};

    println!("cargo:rerun-if-env-changed=RINGDESIGNER_OCCT_WORKER");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let generated = out.join("occt_worker.rs");
    let Some(worker) = env::var_os("RINGDESIGNER_OCCT_WORKER").filter(|v| !v.is_empty()).map(|v| workspace_root().join(v)) else {
        std::fs::write(&generated, "pub static EMBEDDED: Embedded = Embedded::NONE;\n").unwrap();
        return;
    };
    println!("cargo:rerun-if-changed={}", worker.display());
    let raw = std::fs::read(&worker).unwrap_or_else(|e| panic!("RINGDESIGNER_OCCT_WORKER={}: {e}", worker.display()));
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let (magic, kind): (&[&[u8]], &str) = match os.as_str() {
        "windows" => (&[b"MZ"], "a Windows executable"),
        "macos" => (&[&[0xcf, 0xfa, 0xed, 0xfe], &[0xca, 0xfe, 0xba, 0xbe]], "a Mach-O executable"),
        _ => (&[b"\x7fELF"], "an ELF executable"),
    };
    assert!(
        magic.iter().any(|m| raw.starts_with(m)),
        "RINGDESIGNER_OCCT_WORKER={} is not {kind}, which a {os} app needs",
        worker.display()
    );
    let started = std::time::Instant::now();
    let sha256: String = Sha256::digest(&raw).iter().map(|b| format!("{b:02x}")).collect();
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&raw).unwrap();
    let deflated = encoder.finish().unwrap();
    let packed = out.join("occt-worker.deflate");
    std::fs::write(&packed, &deflated).unwrap();
    let packed = packed.to_str().expect("OUT_DIR is UTF-8");
    std::fs::write(
        &generated,
        format!("pub static EMBEDDED: Embedded = Embedded {{ deflated: include_bytes!({packed:?}), sha256: {sha256:?}, bytes: {} }};\n", raw.len()),
    )
    .unwrap();
    let mb = |n: usize| n as f64 / 1e6;
    println!(
        "cargo:warning=ringdesigner: carries the OpenCascade worker {}: {:.1} MB deflated to {:.1} MB in {:.1} s, SHA-256 {sha256}",
        worker.display(),
        mb(raw.len()),
        mb(deflated.len()),
        started.elapsed().as_secs_f64()
    );
}

/// The texture server's address and key, compiled in so a jeweller's copy can
/// generate height maps with nothing to configure.
///
/// Same shape as the Mastertech `database` crate's build script: a `.env` read
/// with `dotenvy`, the process environment filling anything it does not carry,
/// and `cargo:rustc-env` under the key's own name so the code says
/// `env!("COMFY_GATE_KEY")`. Unlike that crate nothing here is required —
/// **an empty value means the feature is off at run time**, and a build with no
/// `.env` and no environment is a normal build without a texture server.
///
/// The key belongs to a gate account that owns nothing, can read only its own
/// renders and may run one workflow, so it is a public identifier rather than
/// a secret — which is the only reason it can travel in a binary that anyone
/// can run `strings` over. Nothing else about the program may be credentialed
/// this way.
fn texture_gate() {
    const KEYS: [&str; 2] = ["COMFY_GATE_URL", "COMFY_GATE_KEY"];
    let mut map: HashMap<String, String> = HashMap::new();

    for file in gate_env_files() {
        println!("cargo:rerun-if-changed={}", file.display());
        if !file.exists() {
            continue;
        }
        match dotenvy::from_path_iter(&file) {
            Ok(items) => {
                for (key, value) in items.flatten() {
                    if KEYS.contains(&key.as_str()) && !value.is_empty() {
                        map.entry(key).or_insert(value);
                    }
                }
            }
            Err(e) => println!("cargo:warning=ringdesigner: {}: {e}", file.display()),
        }
    }

    // The process environment fills what no file carried — a CI secret, or a
    // one-off `COMFY_GATE_KEY=… cargo build`.
    for key in KEYS {
        println!("cargo:rerun-if-env-changed={key}");
        if map.get(key).is_none_or(String::is_empty) {
            if let Ok(value) = env::var(key) {
                if !value.is_empty() {
                    map.insert(key.to_string(), value);
                }
            }
        }
    }

    if map.get("COMFY_GATE_KEY").is_none_or(String::is_empty) {
        println!("cargo:warning=ringdesigner: no COMFY_GATE_KEY — this build has no texture server");
    }
    for key in KEYS {
        // Always emitted, so `env!` compiles either way; a newline would end
        // the directive and hand cargo the rest of the value as its own.
        let value = map.get(key).filter(|v| !v.contains(['\n', '\r'])).cloned().unwrap_or_default();
        println!("cargo:rustc-env={key}={value}");
    }
}

/// Where a gate environment file may sit, most specific first: an explicit
/// `COMFY_ENV_FILE`, a `.env` at the workspace root, then the desktop's own
/// `~/.config/ringdesigner/comfy.env`.
fn gate_env_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(explicit) = env::var_os("COMFY_ENV_FILE") {
        println!("cargo:rerun-if-env-changed=COMFY_ENV_FILE");
        out.push(PathBuf::from(explicit));
    }
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .and_then(std::path::Path::parent)
        .map(std::path::Path::to_path_buf);
    if let Some(root) = root {
        out.push(root.join(".env"));
    }
    // Declared, or a change of home silently keeps the last build's answer.
    println!("cargo:rerun-if-env-changed=XDG_CONFIG_HOME");
    println!("cargo:rerun-if-env-changed=HOME");
    let config = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    if let Some(config) = config {
        out.push(config.join("ringdesigner").join("comfy.env"));
    }
    out
}

fn resources() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let ico = env::var("DEP_RINGDESIGN_ASSETS_ICO")
        .expect("ringdesign-assets' build script publishes the .ico path");

    let manifest = out.join("ringdesigner.manifest");
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let mut parts = version.split(['.', '-', '+']).filter_map(|p| p.parse::<u16>().ok());
    let (major, minor, patch) = (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    );
    std::fs::write(&manifest, MANIFEST.replace("0.0.0.0", &format!("{major}.{minor}.{patch}.0"))).unwrap();

    let rc = out.join("ringdesigner.rc");
    let mut f = std::io::BufWriter::new(std::fs::File::create(&rc).unwrap());
    // A resource script's strings are C strings; the separators escape.
    let escape = |p: &str| p.replace('\\', "\\\\");
    writeln!(f, "1 ICON \"{}\"", escape(&ico)).unwrap();
    writeln!(f, "1 24 \"{}\"", escape(&manifest.to_string_lossy())).unwrap();
    writeln!(f, "1 VERSIONINFO").unwrap();
    writeln!(f, "FILEVERSION {major},{minor},{patch},0").unwrap();
    writeln!(f, "PRODUCTVERSION {major},{minor},{patch},0").unwrap();
    writeln!(f, "FILEOS 0x4\nFILETYPE 0x1").unwrap();
    writeln!(f, "{{\n BLOCK \"StringFileInfo\"\n {{\n  BLOCK \"040904b0\"\n  {{").unwrap();
    for (key, value) in [
        ("CompanyName", "Kings of Alchemy"),
        ("FileDescription", "RingDesigner — procedural sand-castable ring design"),
        ("FileVersion", version.as_str()),
        ("InternalName", "ringdesigner"),
        ("OriginalFilename", "ringdesigner.exe"),
        ("ProductName", "RingDesigner"),
        ("ProductVersion", version.as_str()),
    ] {
        writeln!(f, "   VALUE \"{key}\", \"{value}\\0\"").unwrap();
    }
    writeln!(f, "  }}\n }}\n BLOCK \"VarFileInfo\"\n {{\n  VALUE \"Translation\", 0x409, 1200\n }}\n}}").unwrap();
    drop(f);

    embed_resource::compile(&rc, embed_resource::NONE).manifest_required().unwrap();
}

/// `asInvoker`, deliberately: a design program has nothing to elevate for, and
/// an elevated process cannot accept a file dragged onto it from Explorer —
/// which is how a `.ring.json` is opened. `longPathAware` and per-monitor DPI
/// keep long export paths and a 4K screen honest.
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="com.kingsofalchemy.ringdesigner" version="0.0.0.0"/>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
      <supportedOS Id="{1f676c76-80e1-4239-95bb-83d0f6d0da78}"/>
      <supportedOS Id="{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </windowsSettings>
  </application>
</assembly>
"#;
