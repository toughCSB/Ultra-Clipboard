# Reopen Onboarding Entry

## Goal

Add a manual action in preferences so users can reopen the first-run onboarding window to review permissions, import options, and basic usage guidance.

## What I Already Know

- The frontend already has an `/onboarding` route.
- The command layer already wraps `openOnboarding()`, and Rust already exposes `open_onboarding`.
- Preferences already has an action-control pattern suitable for one-shot actions.
- This requirement does not need another persisted settings field.

## Requirements

- Add a "Reopen onboarding" action to the interface/control area of preferences.
- Reuse the existing `openOnboarding()` command when the action is clicked.
- Do not change `onboarding.completed` or reset settings or history.
- Complete ko-KR and en-US copy, search keywords, and icon mapping.

## Acceptance Criteria

- [ ] Preferences displays the "Reopen onboarding" action.
- [ ] Clicking it opens the onboarding window.
- [ ] Clicking it does not reset onboarding to incomplete.
- [ ] Korean and English copy, search, and icons work correctly.
- [ ] Frontend lint and typecheck pass.

## Out of Scope

- Do not redesign the onboarding flow.
- Do not add settings fields.
- Do not change first-run automatic onboarding behavior.
