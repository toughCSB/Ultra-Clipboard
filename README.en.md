<div align="center">
  <img src="./public/logo.png" alt="Ultra Clipboard" width="96" height="96" />

# Ultra Clipboard

**A local-first clipboard manager for macOS and Windows.**

[한국어](./README.md) | English | [简体中文](./README.zh-CN.md)

  <br />

  <img alt="Tauri v2" src="https://img.shields.io/badge/Tauri-v2-24c8db?style=flat-square" />
  <img alt="Rust first" src="https://img.shields.io/badge/Rust-first-b7410e?style=flat-square" />
  <img alt="React 19" src="https://img.shields.io/badge/React-19-61dafb?style=flat-square" />
  <img alt="macOS" src="https://img.shields.io/badge/macOS-supported-000000?style=flat-square&logo=apple&logoColor=white" />
  <img alt="Windows" src="https://img.shields.io/badge/Windows-supported-0078d4?style=flat-square&logo=windows&logoColor=white" />
</div>

## About

Ultra Clipboard is an open source desktop clipboard manager that stores copied content on your device so you can find and reuse it quickly. It uses a Rust-first Tauri architecture with a React UI. Korean is the default UI language.

This project is an independently maintained fork based on [EcoPaste by EcoPasteHub](https://github.com/EcoPasteHub/EcoPaste). Ultra Clipboard is not affiliated with or officially endorsed by EcoPasteHub. Development and releases are maintained independently by `toughCSB`.

## Features

- Capture plain text, HTML, RTF, images, files, and folders
- Search clipboard content and notes with SQLite FTS5
- Filter history by source application and content type
- Display full file paths and reveal recorded files
- Preview text, images, and files
- Paste, copy, copy as plain text, open links, add notes, favorite, pin, and delete items
- Organize history with favorites, pinned items, notes, and custom groups
- Optional copy sound feedback
- Protect against collecting or displaying high-confidence secrets such as private keys, service tokens, AWS keys, and JWTs
- One-month default retention, approximately 30 days. The period can be changed in settings, and favorites and pinned items are excluded from automatic cleanup
- Local-first storage for clipboard data, resources, and settings

## Backup and Data Compatibility

Ultra Clipboard can export and import `.ecopastebak` backups. Encrypted `.ecopastebak` backups are supported as well. Backups contain clipboard history, resources, and settings, and can be restored by merging with or overwriting current data.

WebDAV backup and restore are manual actions available in settings. They are not real-time synchronization. Ultra Clipboard does not provide real-time Tailscale synchronization. Only HTTPS WebDAV endpoints are supported, and the WebDAV password is excluded from backup files.

Ultra Clipboard uses a separate data namespace from EcoPaste. To bring data from the official EcoPaste app, create an `.ecopastebak` backup in EcoPaste and import it through Ultra Clipboard's backup flow. Existing data is not migrated automatically.

## Platform Support

- Windows
- macOS

Linux is not supported. `v1.1.1-rc.1` is a prerelease, and macOS runtime QA has not yet been completed.

## Download and Installation

Download the latest builds from [Releases](https://github.com/toughCSB/Ultra-Clipboard/releases).

`v1.1.1-rc.1` is currently unsigned. Windows SmartScreen or macOS Gatekeeper may show a warning. Verify the source and file before using it. This release does not claim code signing or notarization.

## Development

You need macOS or Windows, Node.js 20 or newer, pnpm 10 or newer, and a Rust toolchain.

```bash
pnpm install
pnpm tauri dev
pnpm tauri build
```

Run frontend and Rust checks with:

```bash
pnpm lint
pnpm tsc
cd src-tauri
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

See the [contribution guide](./CONTRIBUTING.md) for development rules and repository structure.

## License

Ultra Clipboard is distributed under the [Apache License 2.0](./LICENSE). Original copyright notices are retained, and fork modifications are listed separately in [NOTICE](./NOTICE).
