# Journal - Codex (Part 1)

> AI development session journal
> Started: 2026-09-19

---



## Session 1: 자동 시작, 캡처, 삭제 버튼 QA

**Date**: 2026-09-19
**Task**: 자동 시작, 캡처, 삭제 버튼 QA
**Branch**: `toughCSB/autostart-perf-trash` → `release/v1.2.2`

### Summary

HKCU 자동 시작 수정과 QA On/Off 확인; 캡처 페인트 게이트 수정 및 성능 경로 조사; 삭제 버튼 양 테마 WebView 확인. 캡처 시각 QA는 데스크톱 핸들 오류로 남음.

### Main Changes

- Windows 자동 실행에서 HKLM 접근을 제거하고 HKCU 상태만 읽고 갱신하도록 수정.
- 캡처 오버레이의 첫 창 중복 세션 요청과 표시 전 bounds 적용 순서를 수정.
- 클립보드 삭제 버튼을 28×28px로 키우고 danger 토큰을 적용.
- Orca 작업물 30개 파일을 D: 저장소로 이전하고 SHA-256을 대조.

### Git Commits

- `2d82055` Windows 자동 실행 사용자 범위 수정.
- `f27d464` 캡처 오버레이 순서와 타임아웃 수정.
- `cbcd1c3` 삭제 버튼 가시성 개선.

### Testing

- `corepack pnpm lint`: 237개 파일 통과.
- `corepack pnpm exec tsc --noEmit`: 통과.
- `corepack pnpm test`: 46개 통과.
- `corepack pnpm build`: 통과.
- `cargo fmt --check`, `cargo clippy -- -D warnings`: 통과.
- `cargo test`: 229개 통과, 8개 ignored.
- 별도 QA 앱의 온보딩과 클립보드 창 진입을 확인했으나, Orca 화면 캡처와 창 포커스 실패로 캡처 깜빡임 시각 검증은 완료하지 못함.

### Status

**In review**

### Next Steps

- 출시 후 실제 Windows 화면에서 캡처 깜빡임과 다중 모니터 동작을 확인.
- macOS 런타임 경로는 macOS 장비에서 확인.
