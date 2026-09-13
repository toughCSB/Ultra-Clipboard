# 변경 기록

## [1.1.2](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.1.1...v1.1.2) (2026-09-13)

### 변경

- 작업표시줄에서 더 크게 보이도록 확정된 `UC` 그림의 아이콘 canvas 점유 면적을 확대했습니다.
- UI, installer, 문서와 release 지원 언어를 한국어와 영어로 정리했습니다.
- 기존 `zh-CN` 설정을 한국어로 변환하고 중국어 UI resource를 제거했습니다.
- 원본에서 이어진 중국어 source comment와 native fallback 문구를 영어 또는 한국어로 바꿨습니다.
- GitHub release 설명을 영어 다음 한국어 순서로 생성합니다.

### 릴리즈 안내

- 이 릴리즈는 서명되지 않았으며 Windows SmartScreen과 macOS Gatekeeper가 경고를 표시할 수 있습니다.
- 자동 업데이트는 비활성화되어 있습니다. 이후 버전은 GitHub Releases에서 내려받으세요.
- Tailscale 실시간 클립보드 동기화는 포함되지 않았습니다. WebDAV 백업과 복원은 계속 수동으로 실행합니다.
- macOS Intel 및 Apple Silicon 패키지는 CI에서 빌드하지만 macOS runtime QA는 아직 완료되지 않았습니다.

## [1.1.1](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.1.1-rc.1...v1.1.1) (2026-09-13)

### 변경

- 선택한 `UC` 클립보드 마크를 Ultra Clipboard 앱과 트레이 아이콘에 최종 적용했습니다.
- 투명 PNG 원본 이미지에서 데스크톱 앱 아이콘을 생성하도록 변경했습니다.

### 릴리즈 안내

- 이 릴리즈는 서명되지 않았으며 Windows SmartScreen과 macOS Gatekeeper가 경고를 표시할 수 있습니다.
- 자동 업데이트는 비활성화되어 있습니다. 이후 버전은 GitHub Releases에서 내려받으세요.
- macOS Intel 및 Apple Silicon 패키지는 CI에서 빌드하지만 macOS runtime QA는 아직 완료되지 않았습니다.
- EcoPaste 백업 호환성을 위해 `.ecopastebak` 형식을 계속 지원합니다.

## [1.1.1-rc.1](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.1.0...v1.1.1-rc.1) (2026-09-12)

### 기능

- 독립 fork를 별도 앱 identity와 새 아이콘을 사용하는 Ultra Clipboard로 변경했습니다.
- 한국어를 기본 UI 언어로 추가했습니다.
- 수동 WebDAV 백업 업로드와 복원을 추가했습니다.
- 파일 클립보드 카드에 전체 경로를 표시합니다.
- 기본 보존 기간을 한 달로 설정하고 즐겨찾기와 고정 항목은 보존합니다.
- Windows에서 안정적으로 동작하는 native 복사 알림음을 추가했습니다.
- 콘텐츠 유형에 맞는 강조색으로 클립보드와 설정 화면을 개선했습니다.

### 릴리즈 안내

- 이 release candidate는 서명되지 않았으며 Windows SmartScreen과 macOS Gatekeeper가 경고를 표시할 수 있습니다.
- 자동 업데이트는 비활성화되어 있습니다. 이후 버전은 GitHub Releases에서 내려받으세요.
- macOS Intel 및 Apple Silicon 패키지는 CI에서 빌드하지만 macOS runtime QA는 아직 완료되지 않았습니다.
- EcoPaste 백업 호환성을 위해 `.ecopastebak` 형식을 계속 지원합니다.
