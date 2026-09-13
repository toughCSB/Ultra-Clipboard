# Auto Updater

## Goal

Integrate Tauri v2 automatic updates with update checks, downloads, installation prompts, and signature verification.

## Scope

- Add `tauri-plugin-updater`.
- Configure update endpoints.
- Configure the `TAURI_SIGNING_PRIVATE_KEY` environment variable.
- Provide update-check UI in preferences or the About page.
- Provide a separate Software Update window for checking, available, downloading, current, and error states.
- Connect `update.autoCheck` and `update.includeBeta` settings to update behavior.

## Implementation Notes

- Configure the plugin, capability, and `tauri.conf` according to the official Tauri v2 updater documentation.
- Separate stable and beta update endpoints.
- Log download, installation, and restart paths clearly.
- Open a separate Software Update window from About or preferences. Show current version, update status, and errors; keep release-notes metadata in the backend response without displaying it yet.
- Delay automatic checks after startup to avoid slowing the first screen.
- Report failures through unified command error handling and logs.

## Acceptance Criteria

- A local mock endpoint can report a new version.
- Installation is rejected when the signature does not match.
- Beta packages are not offered when `includeBeta` is disabled.
- Users can open the Software Update window from preferences or About and check, download, install and restart, or skip the current version.
- macOS and Windows packages use the same update flow.
