# Changelog

## [1.2.2](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.2.1...v1.2.2) (2026-09-19)

### Fixed

- Keep Windows login autostart in the current user's registry. Enabling, checking, and disabling the setting no longer access HKLM, which could fail with `0x80070005` under a standard account.
- Wait for the screenshot overlay's first paint before showing it, avoid a duplicate first-session request, and apply window bounds and topmost state before display. An overlay that never paints now times out and cancels the capture instead of showing a blank window.

### Changed

- Enlarge the clipboard card's delete action from 20×20px to 28×28px and give it a clearer danger-colored background, border, and hover state in light and dark themes.

### Release Notes

- Legacy HKLM startup entries are not removed by the current-user setting; removing one requires administrator access.
- Windows capture flicker, mixed-DPI multi-monitor behavior, and macOS runtime behavior could not be visually verified in the available desktop session.
- Updater artifacts are signed with a dedicated Tauri minisign key. Windows and macOS application packages remain code-unsigned and may show SmartScreen or Gatekeeper warnings.

## [1.2.1](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.2.0...v1.2.1) (2026-09-18)

### Added

- Add a delete-and-close action to the screenshot editor that closes the editor without copying the capture to the clipboard.

### Changed

- Refine the Windows tray icon with a red background and a high-contrast white clipboard mark.
- Use Pretendard Black for the app name in the preferences sidebar.

### Fixed

- Fix an update-time hang on Windows: migration `.sql` files could be checked out with CRLF line endings, which changed the embedded sqlx checksum and made an already-applied migration look edited. Migration files are now pinned to LF via `.gitattributes`, and startup safely resyncs a stored checksum when the only difference is CRLF/LF, without weakening the check for a real content change.

## [1.2.0](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.1.3...v1.2.0) (2026-09-17)

### Added

- Add a built-in screen capture and Shottr-class editor: area, fullscreen, window, repeat, and delayed capture from the tray menu and shortcuts.
- Add 13 annotation tools (arrow, text, ruler, rectangle, backdrop, pen, magnifier, blur, highlighter, spotlight, counter, oval, line) with per-tool color icons and a drag-to-reorder toolbar.
- Add a double-click options panel per tool (a single click applies the tool with its last-used settings) and hover tooltips for each toolbar icon.
- Add OCR text recognition, clipboard image paste, and a one-click button that copies to the clipboard and closes the editor window at the same time (Ctrl/⌘+Enter).
- Replace the system tray icon with a bolder, higher-contrast design that stays legible at 16px.

### Changed

- Enlarge the app logo in the preferences sidebar and split the app name across two lines next to it.

### Release Notes

- Updater artifacts are signed with a dedicated Tauri minisign key. Windows and macOS application packages remain code-unsigned and may show SmartScreen or Gatekeeper warnings.
- macOS Intel and Apple Silicon packages are built in CI, but macOS runtime QA — including the screenshot editor — is still pending.

## [1.1.3](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.1.2...v1.1.3) (2026-09-13)

### Added

- Add opt-in real-time text, HTML, RTF, and PNG sharing between explicitly configured Tailscale peers.
- Add in-app update checking, download, signature verification, and installation from the About preferences tab.

### Fixed

- Use a dedicated full-canvas runtime tray icon so the installed Windows tray mark displays at maximum practical size.
- Keep the app running with sync stopped when a peer credential or bind address is unavailable.
- Exclude Tailscale peer configuration and credentials from history backups.

### Release Notes

- Updater artifacts are signed with a dedicated Tauri minisign key. Windows and macOS application packages remain code-unsigned and may show SmartScreen or Gatekeeper warnings.
- Existing `v1.1.2` installations require one manual upgrade to `v1.1.3`; later releases can update in-app.
- macOS Intel and Apple Silicon packages are built in CI, but macOS runtime QA is still pending.

## [1.1.2](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.1.1...v1.1.2) (2026-09-13)

### Changed

- Enlarge the approved `UC` artwork within the desktop icon canvas for better taskbar visibility.
- Retain Korean and English as the supported interface, installer, documentation, and release languages.
- Map existing `zh-CN` settings to Korean without retaining Chinese interface resources.
- Replace inherited Chinese source comments and native fallback messages with English or Korean.
- Generate GitHub release notes in English followed by Korean.

### Release Notes

- This release is unsigned. Windows SmartScreen and macOS Gatekeeper may display warnings.
- Automatic updates remain disabled; download future versions from GitHub Releases.
- Tailscale real-time clipboard synchronization is not included. WebDAV backup and restore remain manual actions.
- macOS Intel and Apple Silicon packages are built in CI, but macOS runtime QA is still pending.

## [1.1.1](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.1.1-rc.1...v1.1.1) (2026-09-13)

### Changed

- Finalize the Ultra Clipboard app and tray icons with the selected `UC` clipboard mark.
- Generate desktop app icons from transparent PNG source artwork.

### Release Notes

- This release is unsigned. Windows SmartScreen and macOS Gatekeeper may display warnings.
- Automatic updates are disabled for this release; download future versions from GitHub Releases.
- macOS Intel and Apple Silicon packages are built in CI, but macOS runtime QA is still pending.
- `.ecopastebak` remains supported for EcoPaste backup compatibility.

## [1.1.1-rc.1](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.1.0...v1.1.1-rc.1) (2026-09-12)

### Features

- Rebrand the independent fork as Ultra Clipboard with a separate application identity and new icon.
- Add Korean as the default interface language.
- Add manual WebDAV backup upload and restore.
- Show full file paths in file clipboard cards.
- Use a one-month default retention period while preserving favorites and pinned items.
- Add reliable native copy-sound feedback on Windows.
- Refresh clipboard and preference layouts with content-aware accents.

### Release Notes

- This is an unsigned release candidate. Windows SmartScreen and macOS Gatekeeper may display warnings.
- Automatic updates are disabled for this release; download future versions from GitHub Releases.
- macOS Intel and Apple Silicon packages are built in CI, but macOS runtime QA is still pending.
- `.ecopastebak` remains supported for EcoPaste backup compatibility.

## [1.1.0](https://github.com/EcoPasteHub/EcoPaste/compare/v1.0.0...v1.1.0) (2026-07-22)

### ✨ Features

* add image save context menu ([#1353](https://github.com/EcoPasteHub/EcoPaste/issues/1353)) ([746cc05](https://github.com/EcoPasteHub/EcoPaste/commit/746cc0558d656ddb694c47c81a46549b89139c79))
* add nightly update channel support ([#1350](https://github.com/EcoPasteHub/EcoPaste/issues/1350)) ([9544725](https://github.com/EcoPasteHub/EcoPaste/commit/95447258f9e443d5285d46b775c7279d3d14fddc))

### 🐛 Bug Fixes

* correct timezone offset for legacy data import ([#1338](https://github.com/EcoPasteHub/EcoPaste/issues/1338)) ([d6ea9ed](https://github.com/EcoPasteHub/EcoPaste/commit/d6ea9ed4d1ce6af99673a53a3c1ed78068f83d53))
* deduplicate Windows autostart entries ([#1355](https://github.com/EcoPasteHub/EcoPaste/issues/1355)) ([0d87458](https://github.com/EcoPasteHub/EcoPaste/commit/0d87458b70323da27456cfaa16b51dcde9ea220e))
* prevent app freezes that may cause history loss ([#1342](https://github.com/EcoPasteHub/EcoPaste/issues/1342)) ([7a75196](https://github.com/EcoPasteHub/EcoPaste/commit/7a75196e90f4a4abfb49d9f84c591d80c735a468))
* quote windows autostart paths ([#1369](https://github.com/EcoPasteHub/EcoPaste/issues/1369)) ([d9d718e](https://github.com/EcoPasteHub/EcoPaste/commit/d9d718e456173e62f112ebd96609c049a331a1e8))
* retry Windows clipboard reads for screenshot capture ([#1367](https://github.com/EcoPasteHub/EcoPaste/issues/1367)) ([e6f7e80](https://github.com/EcoPasteHub/EcoPaste/commit/e6f7e80e83ae18681a976929d710d99119168866))
* show logo for clipboard items without source app ([#1370](https://github.com/EcoPasteHub/EcoPaste/issues/1370)) ([ad9eee0](https://github.com/EcoPasteHub/EcoPaste/commit/ad9eee0f1b7e3abe6546d4b3ac0fc8d4899bd5cb))
* skip invalid legacy import rows ([#1326](https://github.com/EcoPasteHub/EcoPaste/issues/1326)) ([c0f2d58](https://github.com/EcoPasteHub/EcoPaste/commit/c0f2d587677ab51f9c707f1e50fda6a83ce78a3e))
* update beta preference copy ([#1329](https://github.com/EcoPasteHub/EcoPaste/issues/1329)) ([185bb25](https://github.com/EcoPasteHub/EcoPaste/commit/185bb2520e8ce90da8246c0c73b1ce87e54403e5))

## [1.0.0](https://github.com/EcoPasteHub/EcoPaste/compare/v0.6.0-beta.3...v1.0.0) (2026-07-03)

This is a fully refactored version of EcoPaste. The app is lighter, faster, and more stable overall.

### ✨ Features

- EcoPaste can now open on Windows without taking focus from other windows.
- Added a Windows Win+V toggle, so EcoPaste can replace the system clipboard panel.
- Added first-run onboarding for permissions, shortcuts, ignored apps, and legacy data import.
- Added source app detection and ignored apps, so you can control which apps are excluded from clipboard history.
- Added capture preferences for saved content types, size limits, and priority.
- Added sensitive content protection to automatically skip high-risk content such as private keys, tokens, AWS keys, and JWTs.
- Added full content preview for text, image, and file records.
- Added drag-out support for dragging text, rich text, images, and file records into external apps.
- Added deletion protection for favorites and pinned items.
- Added preference search and preference reset.
- Backup import and export now support encrypted backups and merge import.
- Added a Windows administrator launch setting.
- Moved software updates into a dedicated window for checking, downloading, and installing new versions.
- More new features are ready for you to download, try, and explore.

### 🐛 Bug Fixes

- Fixed autostart not working when EcoPaste runs as administrator on Windows.

### ⚡️ Performance

- Clipboard monitoring, search, list rendering, and content preview are faster and more stable.
- App startup, everyday usage, and background resource usage have been further optimized.
- Image thumbnails, app icons, and file icons now load more efficiently.
- Added lightweight mode to reduce background refresh work and memory usage after the window is hidden.

### ⚠️ Upgrade Notice

- This refactored version only supports macOS and Windows.
- This version supports migrating history data from the legacy app during first-run onboarding.
