# Onboarding

## Goal

Show a separate first-run onboarding window with steps for welcome, permissions, shortcuts, ignored applications, legacy-data import, and completion.

## Scope

- Model the onboarding UI after HapiGo's dark borderless setup wizard: large titles, centered content, a primary bottom button, screenshot-style permission guidance, and a completion page.
- Always use dark mode in onboarding, independent of the global theme.
- Set `decorations: false`, use a fixed size, and center the window when opened.
- Use CSS `rounded-4` corners on macOS; keep a regular borderless rectangle on Windows.
- Use a dropdown language switcher in the upper-right corner rather than segmented controls.
- Do not provide a close button before completion, and intercept native close requests for mandatory onboarding.
- Add `Settings.onboarding.completed: bool`, defaulting to false.
- Add `Settings.onboarding.lastStep: number` to resume after interruption.
- Store lightweight legacy-import state under `Settings.onboarding.legacyImport`, including detection, import status, selected types, and time.
- Use the `onboarding` window label and `/#/onboarding` URL.
- Create the window at startup according to `SettingsStore.snapshot().onboarding.completed`.
- Add `pages/Onboarding`.
- Implement each step as a separate component with a central step array. Prefer Ant Design interactions over duplicate primitives.

## Implementation Notes

- Rust commands include `open_onboarding()`, `set_onboarding_step(step)`, `finish_onboarding()`, `check_permissions()`, `detect_legacy_data()`, `import_legacy_data(types)`, and `cancel_legacy_import(task_id)`.
- macOS permission checks cover Accessibility at minimum; show Screen Recording and Input Monitoring only when required.
- Verify local `EcoPaste_bak` data and the legacy identifier before implementing legacy paths.
- Implement the stepper with Ant Design or existing project primitives.
- Keep every step in a separate component and define the step array centrally for extension.
- Reuse the preferences shortcut recorder in the shortcut step.
- Reuse recent source applications and settings writeback in the ignored-applications step.
- Show type counts, total size, progress, and cancellation in the legacy-import step.
- Open the legacy database read-only without running migrations.
- Convert selected types to the new schema; copy images to the current resource directory and recalculate hashes.
- Reuse existing insertion and deduplication logic.
- Run long work as background tasks with progress events so the window is not blocked.

## Acceptance Criteria

- Onboarding opens automatically on first launch and does not reopen automatically after completion.
- The window cannot be hidden or destroyed through a close button or native close request before completion.
- After an unexpected exit, onboarding resumes from the previous step.
- Preferences can reopen onboarding.
- Legacy import can be cancelled and leaves no partial data on failure.
- macOS and Windows both have reasonable fallback paths.
