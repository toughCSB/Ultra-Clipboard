# AGENTS.md

> This file is the single source of truth for AI coding tools in this project. Other tool entry points should reference it instead of duplicating these rules.
> Staged backlog work lives in `.trellis/tasks/`; each task's PRD and research are authoritative for that task.

Ultra Clipboard is a cross-platform clipboard manager based on EcoPaste and uses a Rust-first Tauri architecture.

## Core Principles

- **Rust first**: Put business logic, system capabilities, database access, and persistence in Rust. Keep the frontend focused on presentation and interaction.
- **macOS and Windows only**: Do not add Linux code, dependencies, build artifacts, or documentation promises.
- **Treat released data as public contracts**: Schema, settings, defaults, and migrations require an explicit upgrade path.
- **Evolve the current project directly**: Treat this repository as the implementation baseline and follow its current product and platform constraints.
- **Respect a dirty worktree**: Never overwrite or revert changes you did not make. Read modified files before touching them.
- **Branch policy**: When a push is required from `master`, create a work branch first. On another branch, ask whether to push that branch or create a new one.

## Stack

| Area | Choice |
| --- | --- |
| Desktop | Tauri v2 |
| Frontend | React 19 + Ant Design v6 + UnoCSS `presetWind4` |
| State | Valtio for UI state and settings mirrors only |
| Backend | Rust + sqlx + SQLite |
| Build | Vite + pnpm |
| Quality | Biome, rustfmt, clippy, cargo test |

## Architecture Boundaries

**Implement in Rust**

- Clipboard monitoring, clipboard writeback, simulated paste, and feedback-loop suppression.
- All database reads and writes, SQLite FTS5 search, and history cleanup.
- Content detection for URLs, email addresses, colors, and paths.
- Window positioning, OS-level keyboard hooks, global shortcuts, tray behavior, and autostart.
- Image persistence, thumbnails, file metadata, and settings persistence.
- Short Rust-owned user-visible labels for the tray, native context menus, and command toasts belong in `i18n/`. Logs and internal error context do not.

**Keep in the frontend**

- Component rendering, virtual scrolling, masonry layout, animation, and list selection state.
- Theme visuals, CSS variable injection, and frontend i18n rendering.
- Sanitized HTML preview, RTF rendering, and Markdown rendering.
- Normal keyboard interaction. Use Rust `keyboard/` events when the Windows main window cannot receive keys.

**Cross-layer contracts**

- The frontend calls Rust through `#[tauri::command]`; Rust emits refresh events.
- Event names use `domain://action`, such as `clipboard://updated`, `settings://updated`, `window://visibility`, and `keyboard://nav`.
- Reused command names, event names, channels, and storage keys must be centralized as Rust module constants with matching values in `src/constants/`.

## Directory Conventions

```text
src-tauri/
  src/
    commands/   # thin Tauri command validation and dispatch
    db/         # sqlx repositories, connection pool, models
    clipboard/  # clipboard I/O, monitoring, content detection
    window/     # window management, positioning, platform code
    keystroke/  # simulated paste input
    keyboard/   # OS-level keyboard hook on Windows
    mouse/      # global mouse hook and focus-loss hiding on Windows
    shortcut/   # global shortcuts
    tray/       # tray menu
    menu/       # clipboard context menus
    drag_out/   # native drag-out for files, images, and text
    backup/     # `.ecopastebak` import and export
    i18n/       # Rust-owned user-visible labels
    autostart/  # startup registration
    settings/   # settings model and persistence
    core/       # errors, paths, prevent_default
  migrations/
src/            # frontend components, pages, stores, hooks, locales, utilities
```

## Common Commands

```bash
pnpm install
pnpm tauri dev
pnpm tauri build
pnpm lint
pnpm format

cd src-tauri
cargo fmt
cargo clippy -- -D warnings
cargo test
```

## Rust Conventions

- Commands and repositories use `async` and return `Result<T, AppError>`. `AppError` serializes as `{ kind, message }`.
- `message` contains a user-readable cause without an action prefix such as `"xxx failed: {err}"`. The frontend toast supplies action context; logs carry technical context.
- Use `thiserror` for error types, `anyhow` for internal propagation, and `tauri-plugin-log` for context.
- Database code uses Tauri `State<SqlitePool>` rather than opening a new connection per call.
- Prefer major dependency versions in Cargo, or minor versions only when narrowing is necessary. Pin a patch version only with a documented reason.
- Use `sqlx::query` and `query_as`, not `query!`, to avoid offline cache maintenance.
- Add a migration for every released schema change. Never edit a released migration.
- When schema changes, update every matching `SELECT`, `INSERT`, `UPDATE`, `bind`, and test struct literal.
- Tables require `created_at` and `updated_at` as `TEXT NOT NULL`. Clipboard `updated_at` tracks content reuse; metadata changes must not refresh it.
- Keep `commands/` thin: parameter validation followed by lower-layer calls.
- Isolate platform code with `#[cfg(target_os = "macos")]` and `#[cfg(target_os = "windows")]`. Implement both supported platforms or mark the missing platform explicitly.

## Frontend Conventions

**React and components**

- Use `FC<Props>` and destructure `props` inside the function body.
- End destructuring with `...rest` when forwarding remaining properties.
- Prefer React 19 Actions, `use`, `useOptimistic`, and ref as a prop. Do not add `forwardRef`.
- Extract JSX event callbacks into named functions. Use verbs for single actions and `handleXxx` for general events.
- Arrow functions always use braces and explicit `return`.
- Keep `useEffect` synchronous. Use `useMount` and `useUnmount` for asynchronous initialization and `useRef` for cleanup handles.

**State, data, and platform APIs**

- Valtio stores only UI state and settings mirrors. Load business data through Rust commands.
- Use `async`/`await` with `try`/`catch`, not chained `.then()`, `.catch()`, or `.finally()`.
- Express an undefined value as `void 0`.
- Use `@/utils/log`; never call bare `console.*`.
- Import platform and environment checks from `@/utils/is`.
- Use `getCurrentWebviewWindow()`, not `getCurrentWindow()`.

**Style and UI**

- Prefer Ant Design v6 components and standard props such as `open`, `checked`, `disabled`, and `onClick`.
- Customize Ant Design internals through semantic `classNames` and `styles` slots before global `.ant-*` overrides.
- Use UnoCSS. Compose conditional classes with `cn` from `@/utils/cn` and object syntax.
- Use Ant Design token classes for colors. Extend `src/unocss/presetAntdColors.ts` before adding a color.
- Let normal text inherit `text-ant-text`; use `text-ant-secondary` for secondary information.
- Use semantic text sizes: `text-xs`, `text-sm`, `text-base`, and `text-lg`.
- Use Wind4 numeric spacing and sizing, where `1 = 4px`; avoid arbitrary pixel classes and inline pixels.
- Switch the theme through the root `ConfigProvider` algorithm and synchronize `light` or `dark` on `<html>`.

**Content and lists**

- Keep `ko-KR` and `en-US` i18n resources complete and synchronized.
- Use `react-virtuoso` for lists.
- Sanitize HTML with DOMPurify before rendering.

## General Code Rules

- Add documentation only for non-obvious functions or methods. Use multiline JSDoc in TypeScript and consecutive `///` lines in Rust.
- Prefer early returns over nesting the main flow.
- Separate hooks, declarations, effects, semantic phases, and the final `return` with blank lines.
- Comments inside functions explain only hidden constraints, counterintuitive behavior, or workarounds.
- Do not add historical comments, phase-number TODOs, or external line references.
- Do not add abstractions, compatibility shims, or speculative optimization outside the current requirement.
- Use single-line Conventional Commits such as `feat:`, `fix:`, `refactor:`, and `docs:`.
- After UI changes, operate the real path and boundary cases rather than relying only on type checks.

## External Documentation

- Ant Design v6: <https://ant.design/components/overview> and <https://ant.design/docs/react/customize-theme>
- UnoCSS: <https://unocss.dev/> and <https://unocss.dev/presets/wind4>
- Tauri v2: <https://tauri.app/llms-full.txt>

<!-- TRELLIS:START -->

# Trellis Instructions

These instructions are for AI assistants working in this project.

This project is managed by Trellis. The working knowledge you need lives under `.trellis/`:

- `.trellis/workflow.md` — development phases, when to create tasks, skill routing
- `.trellis/spec/` — package- and layer-scoped coding guidelines (read before writing code in a given layer)
- `.trellis/workspace/` — per-developer journals and session traces
- `.trellis/tasks/` — active and archived tasks (PRDs, research, jsonl context)

If a Trellis command is available on your platform (e.g. `/trellis:finish-work`, `/trellis:continue`), prefer it over manual steps. Not every platform exposes every command.

If you're using Codex or another agent-capable tool, additional project-scoped helpers may live in:

- `.agents/skills/` — reusable Trellis skills
- `.codex/agents/` — optional custom subagents

Managed by Trellis. Edits outside this block are preserved; edits inside may be overwritten by a future `trellis update`.

<!-- TRELLIS:END -->
