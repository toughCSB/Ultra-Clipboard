# 자동 실행, 캡처 반응속도, 삭제 버튼 개선

## Goal

Windows와 macOS에서 로그인 자동 실행이 일반 사용자 권한으로 동작하고, 확인된 반응속도 병목을 개선하며, 삭제 버튼을 쉽게 식별하고 누를 수 있게 한다.

## Background

- 원 요청: 지정된 codex-prompt.md의 세 항목과 검증·최종 보고 형식.
- 원 요청은 Orca 브랜치에서 커밋·push·태그·릴리즈를 금지했다. 이후 사용자가 D: 저장소를 최종 작업 폴더로 지정하고 출시까지 승인해 `release/v1.2.2` 브랜치로 이전했다.
- 자동 실행 토글·설정 키·양 플랫폼 백엔드가 이미 있다. Windows 백엔드는 HKLM Run 항목도 읽고 삭제한다.
- 캡처 오버레이는 첫 사용 때 숨겨서 생성하고 이후 재사용한다. 프레임은 바이너리 IPC로 전달한다.
- 삭제 빠른 동작 버튼은 현재 20×20px이다.

## Requirements

- [A] .trellis/tasks/09-19-autostart: 자동 실행 사용자 범위와 양 플랫폼 설정을 검증·수정한다.
- [P] .trellis/tasks/09-19-capture-performance: 캡처와 앱 주요 경로를 점검해 확인된 병목을 개선한다.
- [T] .trellis/tasks/09-19-trash-button: 삭제 버튼의 색, 크기, 클릭 영역을 개선한다.
- 새 의존성을 추가하지 않고 배포판 프로세스·실데이터와 기존 사용자 데이터를 보존한다.
- 측정 가능한 개선에는 변경 전후 수치를 기록한다.

## Acceptance Criteria

- [ ] 세 자식 작업의 요구사항과 검증을 충족한다.
- [ ] pnpm exec biome check, pnpm exec tsc --noEmit, pnpm build, cargo fmt --check, cargo clippy, cargo test의 실제 출력을 요약한다.
- [ ] Windows 실사용 경로를 확인하고 macOS 미검증 부분을 구분한다.
- [ ] 최종 응답은 요청한 1/2/3 형식과 git status --short, git diff --stat 출력을 포함한다.
