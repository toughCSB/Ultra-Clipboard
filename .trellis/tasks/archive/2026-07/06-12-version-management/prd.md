# Version Management and release-it

## Goal

Configure a unified version-release command for stable, RC, and beta releases while keeping `package.json`, Tauri configuration, and release tags aligned.

## Scope

- Integrate `release-it` or an equivalent release tool.
- Provide `release`, `release-rc`, and `release-beta` commands.
- Drive versions and the changelog from Conventional Commits.
- Keep tag naming aligned with CI trigger rules.

## Implementation Notes

- The current `package.json` version, `0.6.0-beta.3`, marks the unreleased development boundary.
- Changing to another version also changes the data-compatibility policy.
- Confirm the database schema and migration strategy before changing versions.

## Acceptance Criteria

- A dry run outputs the correct version, tag, and changelog.
- Package metadata and the Tauri bundle version match for stable, RC, and beta releases.
- Generated tags trigger CI correctly.
