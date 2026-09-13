---
name: sync-ko-changelog
description: "Synchronize an Ultra Clipboard release section from CHANGELOG.md into CHANGELOG.ko-KR.md with project terminology and structural validation. Use when the user explicitly invokes $sync-ko-changelog or asks to generate, translate, update, or verify the Korean changelog for a release."
---

# Sync Korean Changelog

Synchronize one Ultra Clipboard release from `CHANGELOG.md` to
`CHANGELOG.ko-KR.md` without changing release metadata or unrelated files.

## Trellis routing

- Treat an explicit `$sync-ko-changelog` invocation as the user's standing
  choice to skip Trellis task creation for this narrow documentation change.
  Do not ask again, and do not create, activate, or archive a Trellis task.
- When this skill is selected implicitly from natural-language intent, follow
  the normal Trellis task-creation routing.
- Still respect `AGENTS.md`, the dirty worktree, and applicable release specs.

## Workflow

1. Read both changelog files before editing. Use the version named by the user;
   otherwise select the first release section in `CHANGELOG.md`.
2. If that version already exists in `CHANGELOG.ko-KR.md`, update it in place.
   Otherwise insert it immediately after the `# 변경 기록` title. Never create
   a duplicate section.
3. Translate release prose and headings into concise Korean.
   Search `src/locales/ko-KR/`, `src/locales/en-US/`, and existing Korean
   changelog entries with `rg` when product terminology is unclear.
4. Preserve the source release header's version, compare URL, date, section
   order, entry order, issue numbers, commit hashes, and all link targets.
   Translate only human-readable prose. Keep platform and product names such as
   EcoPaste, macOS, Windows, Token, AWS Key, and JWT consistent with the UI.
5. Edit only `CHANGELOG.ko-KR.md`. Do not modify `CHANGELOG.md`, version files,
   release configuration, or unrelated dirty changes. Do not commit or push.
6. Validate the result from the repository root:

   ```bash
   python3 .agents/skills/sync-ko-changelog/scripts/check_sync.py
   git diff --check -- CHANGELOG.ko-KR.md
   ```

   Pass `--version <version>` to validate a user-selected non-latest release.
7. Fix every validation error, rerun both checks, then report the synchronized
   version and validation result.

## Translation headings

Use these established mappings when the source headings occur:

- `✨ Features` -> `✨ 기능`
- `🐛 Bug Fixes` -> `🐛 버그 수정`
- `⚡️ Performance` -> `⚡️ 성능 개선`
- `⏪ Reverts` -> `⏪ 되돌리기`
- `⚠️ Upgrade Notice` -> `⚠️ 업그레이드 안내`
- `Changed` -> `변경`
- `Release Notes` -> `릴리즈 안내`

For an unfamiliar heading, translate it faithfully while preserving any emoji
and its position.
