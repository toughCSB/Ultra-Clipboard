# 변경 기록

## [1.2.1](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.2.0...v1.2.1) (2026-09-18)

### 수정

- Windows에서 업데이트 후 앱이 멈추는 문제를 수정했습니다: 마이그레이션 `.sql` 파일이 CRLF로 체크아웃되면 sqlx가 내장하는 체크섬이 달라져, 이미 적용된 마이그레이션이 수정된 것처럼 보였습니다. `.gitattributes`로 마이그레이션 파일의 줄바꿈을 LF로 고정하고, 시작할 때 저장된 체크섬이 CRLF/LF 차이로만 다른 경우 안전하게 재동기화하도록 했습니다. 실제 내용이 바뀐 경우를 막는 원래 검사는 그대로 유지됩니다.

## [1.2.0](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.1.3...v1.2.0) (2026-09-17)

### 추가

- 화면 영역/전체화면/창 캡처와 반복·지연 캡처를 트레이 메뉴와 단축키로 실행할 수 있는 Shottr급 캡처 편집기를 추가했습니다.
- 화살표, 텍스트, 자, 사각형, 배경, 펜, 돋보기, 흐리게, 형광펜, 스포트라이트, 번호, 타원, 선 등 13종 주석 도구를 추가했고, 도구마다 구분되는 색상 아이콘과 드래그로 재배치 가능한 툴바를 제공합니다.
- 도구를 더블클릭하면 옵션 패널이 열리고(단일 클릭은 기존 설정으로 즉시 적용), 아이콘에 마우스를 올리면 툴팁으로 안내합니다.
- 텍스트 인식(OCR), 클립보드 이미지 붙여넣기, 클립보드로 복사와 동시에 창을 닫는 원클릭 버튼(Ctrl/⌘+Enter)을 추가했습니다.
- 16px에서도 또렷하게 보이도록 시스템 트레이 아이콘을 더 굵고 대비가 높은 디자인으로 교체했습니다.

### 변경

- 설정 사이드바의 앱 로고를 확대하고, 앱 이름을 큰 아이콘 옆에 두 줄로 표기했습니다.

### 릴리즈 안내

- updater artifact는 전용 Tauri minisign key로 서명합니다. Windows와 macOS 앱 package 자체는 code signing되지 않아 SmartScreen 또는 Gatekeeper 경고가 표시될 수 있습니다.
- macOS Intel 및 Apple Silicon 패키지는 CI에서 빌드하지만, 스크린샷 에디터를 포함한 macOS runtime QA는 아직 완료되지 않았습니다.

## [1.1.3](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.1.2...v1.1.3) (2026-09-13)

### 추가

- 명시적으로 설정한 Tailscale peer 사이에서 텍스트, HTML, RTF와 PNG를 실시간 공유할 수 있습니다.
- 정보 설정 탭에서 업데이트 확인, 다운로드, 서명 검증과 설치를 실행할 수 있습니다.

### 수정

- 별도의 full-canvas runtime tray icon을 사용해 설치된 Windows tray mark가 실용적인 최대 크기로 표시됩니다.
- peer credential 또는 bind address를 사용할 수 없어도 앱은 실행하고 sync만 중지합니다.
- 기록 백업에서 Tailscale peer 설정과 credential을 제외합니다.

### 릴리즈 안내

- updater artifact는 전용 Tauri minisign key로 서명합니다. Windows와 macOS 앱 package 자체는 code signing되지 않아 SmartScreen 또는 Gatekeeper 경고가 표시될 수 있습니다.
- 기존 `v1.1.2` 설치는 `v1.1.3`으로 한 번 수동 업그레이드해야 하며, 이후 릴리즈부터 앱 안에서 업데이트할 수 있습니다.
- macOS Intel 및 Apple Silicon 패키지는 CI에서 빌드하지만 macOS runtime QA는 아직 완료되지 않았습니다.

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
