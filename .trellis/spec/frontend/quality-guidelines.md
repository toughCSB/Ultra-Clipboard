# Quality Guidelines

## Commands

Use these checks for frontend work:

```bash
pnpm lint
pnpm tsc
```

`pnpm lint` runs Biome. `pnpm tsc` checks TypeScript without emitting. There is
no frontend test runner configured in this repository, so UI changes also need
a manual app run through the affected path.

For full cross-layer changes, also run Rust checks:

```bash
cd src-tauri
cargo fmt
cargo clippy -- -D warnings
cargo test
```

## Biome Rules That Matter

`biome.json` enforces sorted imports/classes/properties, no unused imports or
variables, no `console.*`, self-closing JSX, and strict unused template literal
cleanup. `noDangerouslySetInnerHtml` is disabled for narrow, sanitizer-owned
escape hatches; page components should still avoid direct raw HTML injection.

Use `@/utils/log` instead of console logging. Keep imports organized by Biome
rather than hand-sorting in a different style.

## Required Frontend Patterns

- Call Tauri only through wrappers in `src/commands/index.ts`.
- Use `async`/`await` and `try`/`catch`; avoid `.then()` chains in new code.
- Use `void 0` for intentionally absent optional fields when matching local
  style, as in `Clipboard/List.tsx`.
- Keep i18n in both `src/locales/ko-KR/` and `src/locales/en-US/`.
- Use `getCurrentWebviewWindow()` for the current window.
- Use `cn` for conditional classes.
- Sanitize user-provided HTML/SVG with DOMPurify at the rendering boundary.

## UI Verification

### Screenshot window startup

`src/router/index.ts` keeps route components lazy so a new screenshot overlay or
editor WebView does not parse unrelated clipboard and preference pages. The
top-level `Suspense` in `src/main.tsx` covers route loading. Check the production
bundle output when changing these imports; an eager page import can silently
restore the single large startup bundle.

`ScreenshotOverlay` receives raw RGBA from the existing Rust command, paints it
with `ImageData`/`putImageData`, and gives the painted frame canvas to the loupe.
Rust retains the original RGBA for selection cropping, so preview painting must
not change exported pixels. Report overlay readiness only after both canvases
are painted. Measure frame transfer, canvas painting, and editor first paint on
the actual Windows and macOS apps before claiming capture latency improved.

For UI changes, manually verify the main path and at least one boundary case:

- Clipboard list: empty state, search, scroll pagination, pinned/favorite item,
  hidden-window update behavior if events changed.
- Preview: text, HTML/RTF, image, files, missing files, sensitive redaction.
- Preferences: setting commit, reset, search, both language files, relevant side
  effects such as shortcut/tray/autostart.
- Window changes: main, preference, and preview windows on the affected platform.

### macOS permission display

`tauri-plugin-macos-permissions` 2.3.0 implements
`checkFullDiskAccessPermission()` by attempting `read_dir` on
`~/Library/Containers/com.apple.stocks` and `~/Library/Safari`. Its boolean is
only the result of those probes; it is not the Full Disk Access switch in macOS
System Settings. The installed app can show false while System Settings shows
Ultra Clipboard enabled, including after an app restart. Render Full Disk Access
as an action that opens `Privacy_AllFiles`, without a checked/unchecked claim or
status polling. Keep the separate screen recording and accessibility checks for
their respective controls. Verify the preferences UI and System Settings on a
Mac before claiming that an actual grant is effective.

If a change affects macOS NSPanel timing or Windows non-focusable keyboard
navigation, manual desktop validation is required. Type checks cannot cover
those paths.

## Review Checklist

- Does the layer ownership match the Rust-first boundary?
- Are command/event names centralized and mirrored?
- Are new user-visible strings present in both locales?
- Does the UI render backend-prepared fields instead of recomputing business
  rules?
- Does a hidden or dormant window need deferred work instead of immediate IPC?
- Are Ant Design token colors used instead of hard-coded colors?
- Did the change avoid global `.ant-*` overrides unless no semantic slot exists?

## Scenario: Release Versioning

### 1. Scope / Trigger

- Trigger: changes to release commands, version metadata, generated release
  notes, or tag names. This is release infrastructure, so package metadata,
  Tauri bundle metadata, Cargo metadata, and CI tag expectations must stay
  aligned.

### 2. Signatures

- `pnpm release` -> stable release via `release-it`.
- `pnpm release-rc` -> `release-it --preRelease=rc --preReleaseBase=1`.
- `pnpm release-beta` -> `release-it --preRelease=beta --preReleaseBase=1`.
- `@release-it/bumper` writes release-it's selected version to
  `src-tauri/Cargo.toml` at `package.version` and to
  `src-tauri/Cargo.lock` at `package.0.version`.

### 3. Contracts

- `package.json.version` is the version source for Tauri because
  `src-tauri/tauri.conf.json` uses `"version": "../package.json"`.
- Release tags must use `v${version}`.
- `CHANGELOG.md` is generated by release-it and is the conventional-changelog
  `infile`.
- `release-it` must not publish to npm; Tauri/GitHub release work is handled by
  CI/release workflows.
- `@release-it/conventional-changelog` must use `ignoreRecommendedBump: true`
  so changelog generation does not replace release-it's interactive version
  choices.

### 4. Validation & Error Matrix

- Missing or non-string `package.json.version` -> fail before syncing.
- Missing Cargo version path -> bumper must fail instead of silently leaving
  Rust metadata stale.
- Dirty worktree on real release -> release-it must stop before writing,
  committing, tagging, or pushing.

### 5. Good/Base/Bad Cases

- Good: `pnpm release-beta` opens release-it's version prompt with beta choices.
- Base: `pnpm release-rc` opens release-it's version prompt with RC choices.
- Bad: letting the conventional-changelog recommended bump choose the prerelease
  base can jump to an unintended prerelease version after feature commits.

### 6. Tests Required

- Run `pnpm lint` and `pnpm tsc` after changing release scripts.
- Run `pnpm release --dry-run --ci --no-git.push` on a clean tree, or add
  `--no-git.requireCleanWorkingDir` only while validating uncommitted local
  changes.
- Verify dry-run output includes the expected `npm version`, bumper writes for
  both Cargo files, release commit message, and `v${version}` tag.
- Verify current metadata alignment: `package.json.version`,
  `src-tauri/Cargo.toml`, the root `EcoPaste` entry in `src-tauri/Cargo.lock`,
  and Tauri's package-json version pointer.

### 7. Wrong vs Correct

#### Wrong

```json
"release-beta": "release-it --preRelease=beta"
```

Without `preReleaseBase`, the prerelease prompt can use the package manager's
default base. Without `ignoreRecommendedBump: true`, conventional-changelog can
replace the prompt with its recommended bump.

#### Correct

```json
"release-beta": "release-it --preRelease=beta --preReleaseBase=1"
```

The command stays interactive while keeping changelog generation separate from
version selection.
