# 로그인 자동 실행

## Goal

설정 화면의 자동 실행 토글이 일반 사용자 범위에서 Windows와 macOS 로그인 실행을 제어한다.

## Background

- src/pages/Preference/config/preferenceSchema.ts:601에 토글이 있다.
- General.auto_start 기본값은 false이고 serde 기본값으로 누락 키를 처리한다.
- src-tauri/src/autostart/windows.rs는 현재 HKLM Run 삭제를 시도해 접근 거부가 발생할 수 있다.
- macOS는 auto-launch의 LaunchAgent 경로를 사용한다.

## Requirements

- Windows 자동 실행의 생성, 확인, 제거는 HKCU 사용자 범위만 사용한다.
- 기존 토글, 설정 키, 기본값, ko-KR/en-US 라벨을 활용한다.
- macOS 기존 로그인 경로를 유지하고 정적 검증한다.
- 토글 On/Off 시 설정과 OS 상태가 일치하며 오류를 숨기지 않는다.

## Acceptance Criteria

- [x] Windows On/Off가 HKCU Run 값과 설정에 반영되고 HKLM 변경 코드가 없다.
- [x] 0x80070005를 일으킨 시스템 Run 삭제 경로가 제거된다.
- [x] 누락 설정 키의 기본값 off가 확인된다.
- [x] macOS cfg 경로를 검토하고 실행할 수 없는 부분을 보고한다.

## Out of Scope

- 과거 HKLM 항목의 자동 삭제. 기존 항목이 남아 있으면 사용자 권한으로 제거할 수 없다.
