# Android 0.19.0 — files, inspector size and menu clearance

Published to [the private app store](https://appstore.shadowbroker.app) on
2026-09-18. Version code 16782080; ARM64 APK 21,590,911 bytes. The downloaded
store APK matches the signed release, and its signer matches 0.18.1.

- **File** is always on the top bar. It opens New, saved designs, Save,
  a Downloads copy, clipboard commands, and the rename/export/share browser.
- **File → New design → Imported signet base** starts editable stock; the
  **Shape → Imported signet base** selector contains 19 masters. The same New
  menu offers Nocturne/Solstice with imported shoulders and four showcase graphs.
  All geometry and template assets are embedded in the APK for offline use.
- The portrait bottom inspector measures its contents, shrinking after controls
  collapse and scrolling at half the available workspace height. The landscape
  side inspector retains its width grip.
- Opening a menu, submenu or tool palette shifts the ring toward clear space
  after layout settles. Closing leaves the camera alone. Manual navigation
  cancels pending movement; zoom and orientation are preserved. If no clear
  region fits the ring, it maximizes visible area at the current zoom.

140 Rust tests passed (122 Android, 18 workbench). ARTEMIS passed seven file,
template and layout checks. The final layout build was then checked directly
with named controls and saved layout reports: short inspectors measured 99 and
133 points, while a long guide stopped at 372 of 745 available points. Manual
pan remained identical after settling, closing caused no adjustment, and
reopening caused exactly one adjustment without changing zoom or orientation.
320-point portrait and landscape header/menu bounds passed. Density changes
were followed by a cold launch to confirm the actual egui scale.

- [Compact inspector](../target/file-menu-review/layout/compact-stones.png)
- [Ring below the File menu](../target/file-menu-review/layout/file-open-autopan.png)
- [New menu and imported templates](../target/file-menu-review/layout/new-menu-autopan.png)
- [Release receipt](../target/releases/ringdesigner-android-0.19.0/release.json)

APK checks include all 19 embedded base files, both imported template files,
launcher/ABI/version, signature, and 16 KB ELF/ZIP alignment. Runtime checks used
the x86_64 emulator; a physical ARM64 phone remains the user's trial. The original
emulator project and preferences were restored byte-for-byte. Imported geometry
behavior and limits are documented in [SIGNET-BASES.md](SIGNET-BASES.md).
