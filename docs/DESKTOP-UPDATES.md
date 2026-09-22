# Desktop releases

RingDesigner checks the public `shadowbrok3r/RingDesigner` GitHub release feed on startup and every six hours. The Updates menu can disable automatic checks or request one immediately. Checks and downloads run off the UI thread. Offline, rate-limited, empty and incompatible feeds leave the current application usable and report their status in the menu.

Only stable `desktop-v<semver>` tags newer than the installed desktop Cargo version are eligible. Android tags, drafts, prereleases and packages for another architecture are ignored. Packages are raw executables:

| Platform | Asset |
| --- | --- |
| Linux x86_64 | `ringdesigner-x86_64-unknown-linux-gnu` |
| Windows x86_64 | `ringdesigner-x86_64-pc-windows-msvc.exe` |
| macOS Apple Silicon | `ringdesigner-aarch64-apple-darwin` |
| macOS Intel | `ringdesigner-x86_64-apple-darwin` |

The downloader requires the asset's GitHub SHA-256 digest or its matching `<asset>.sha256` sidecar. Size is bounded to 512 MiB; short, oversized, corrupt and non-native downloads cannot install. Temporary files are removed on failure or exit. Verification runs again immediately before replacement.

**Install and restart** becomes available after verification, once builds and exports finish. The application embeds custom alphas and saves the design, dock and all workspace layouts before replacing itself using `self-replace`. A write/verification failure leaves the session open. After normal eframe shutdown flushes persistence, the new binary launches without stale `--open` arguments. Read-only installation locations need a user-writable installation; the app reports the failure without elevation.

Desktop 0.3.0 uses the stable application ID `RingDesigner Desktop` for persistence. On its first launch it copies the most recently saved session from the earlier version-specific or unversioned storage locations, leaving those originals intact. It never replaces an existing stable session. If migration cannot write its destination, it continues using the original location. This keeps unsaved designs, cameras and resized viewport splits available after an update.

To publish, update `crates/ringdesign-gui/Cargo.toml`, commit the reviewed changes and push the matching `desktop-v<version>` tag. `.github/workflows/desktop-release.yml` checks tag/version agreement, tests and builds all four targets, creates checksum sidecars and publishes only after every build succeeds. A manual workflow run produces downloadable build artifacts without publishing. The first desktop GitHub release has not yet been published.

Native macOS application signing/notarization and Windows code signing are not configured by this workflow. Its artifacts are standalone executables; replacement of signed `.app` bundles or package-managed installations is outside this release format.

References: [GitHub release API](https://docs.github.com/en/rest/releases/releases), [self-replace platform behavior](https://docs.rs/self-replace/latest/self_replace/), [GitHub runner labels](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
