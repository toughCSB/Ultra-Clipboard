# 변경 기록

## [1.2.5](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.2.4...v1.2.5) (2026-09-23)

### 수정

- 관리자 실행을 취소하거나 승격에 실패한 뒤 Windows에서 UAC 창이 반복해서 뜨지 않도록 했습니다. 이전 설정을 복원하고, 승격을 시작할 수 없으면 현재 사용자 권한으로 실행합니다.

### 변경

- 관리자 실행은 선택 기능이며, 이미 관리자 권한으로 실행 중인 앱에 붙여넣을 때 사용한다는 점을 분명히 안내합니다.

### 릴리즈 안내

- Apple Developer ID가 없어 macOS 패키지는 임시 서명됩니다. 업데이트 후 화면 기록·손쉬운 사용 권한을 다시 허용해야 할 수 있습니다. 기존 클립보드 기록과 앱 설정은 보존됩니다.

## [1.2.4](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.2.3...v1.2.4) (2026-09-22)

### 추가

- macOS 14 이상에서 영역·창·전체 화면·지연·반복 캡처를 제공하고, 캡처 편집기와 Vision OCR을 연결했습니다.
- Windows와 macOS에서 새 형광펜 표시와 선택한 표시의 진하기를 조절할 수 있게 했습니다.

### 변경

- 편집기 화면이 그려질 때까지 캡처 화면을 유지해 바탕 화면 깜빡임을 줄이고, 캡처 화면을 따로 불러오며, 주 복사 버튼 옆의 빨간 버리기 아이콘을 더 분명하게 표시합니다.

### 수정

- 유휴 상태에서 정리된 설정 창을 macOS Dock에서 앱을 다시 열 때 재생성합니다.
- macOS 전체 디스크 접근의 신뢰할 수 없는 상태 스위치를 시스템 설정을 여는 버튼으로 바꿨습니다. 이전 검사는 macOS에서 권한이 켜져 있어도 꺼짐으로 표시될 수 있었습니다.
- 전체 디스크 접근을 온보딩 필수 권한에서 제외했습니다. 캡처에는 화면 기록, 빠른 붙여넣기에는 손쉬운 사용 권한이 필요하며, 전체 디스크 접근은 보호된 파일을 다룰 때만 선택적으로 사용합니다.

### 릴리즈 안내

- macOS 빌드는 Apple Developer ID 서명과 공증이 적용되지 않았습니다. 임시 서명한 빌드 사이에서 앱 식별이 달라질 수 있어 이번 업데이트 후 화면 기록·손쉬운 사용 권한을 다시 허용해야 할 수 있습니다. 기존 클립보드 기록과 앱 설정은 보존됩니다.
- v1.2.3은 공개되지 않은 빌드 후보였습니다. v1.2.4가 v1.2.2 이후 첫 공개 업데이트입니다.

## [1.2.3](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.2.2...v1.2.3) (2026-09-22)

### 추가

- macOS 14 이상에서 영역·창·전체 화면·지연·반복 캡처를 제공하고, 기존 캡처 편집기와 Vision OCR을 연결했습니다.
- macOS 온보딩과 설정에 화면 기록 권한 상태를 추가했습니다.
- Windows와 macOS에서 새 형광펜 표시와 선택한 표시의 진하기를 조절할 수 있게 했습니다.

### 변경

- 편집기 첫 화면이 그려질 때까지 캡처 화면을 유지해 바탕 화면 깜빡임을 줄이고, 캡처 화면을 분리해 창 시작 시간을 줄였습니다.
- 편집기의 버리기 동작을 더 큰 빨간 휴지통 아이콘으로 바꾸고 주 복사 버튼을 강조했습니다.

### 수정

- 유휴 상태에서 설정 창이 정리된 뒤 macOS Dock에서 앱을 다시 열어도 설정 창이 생성되도록 했습니다.

### 릴리즈 안내

- Windows와 macOS 앱 패키지 자체는 Apple 또는 Microsoft 코드 서명을 받지 않았습니다. 업데이트 파일은 기존 Tauri 키로 서명합니다. macOS에서 처음 열 때 Gatekeeper 경고가 표시될 수 있고, 업데이트로 임시 서명이 바뀌면 화면 기록·손쉬운 사용·전체 디스크 접근 권한을 다시 허용해야 할 수 있습니다.
- 혼합 배율 화면 좌표와 캡처 전체 흐름의 반복 지연은 자동 검사와 제한된 실기기 검증만 완료했으며, 혼합 배율 하드웨어 동작은 확인하지 못했습니다.

## [1.2.2](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.2.1...v1.2.2) (2026-09-19)

### 수정

- Windows 로그인 자동 실행을 현재 사용자의 레지스트리로 제한했습니다. 설정을 켜거나 확인하거나 끌 때 HKLM에 접근하지 않아 일반 사용자 계정에서 발생할 수 있던 `0x80070005` 오류를 피합니다.
- 스크린샷 오버레이가 첫 화면을 그린 뒤 표시되도록 하고, 첫 세션의 중복 요청을 막았으며, 창 경계와 최상위 상태를 표시 전에 적용했습니다. 화면을 끝내 그리지 못한 오버레이는 빈 창을 표시하는 대신 시간 초과 후 캡처를 취소합니다.

### 변경

- 클립보드 카드의 삭제 버튼을 20×20px에서 28×28px로 키우고, 라이트·다크 테마에서 더 분명한 위험 동작 배경색·테두리·호버 상태를 적용했습니다.

### 릴리즈 안내

- 현재 사용자 설정은 과거의 HKLM 자동 실행 항목을 제거하지 않습니다. 기존 항목을 없애려면 관리자 권한이 필요합니다.
- 사용 가능한 데스크톱 세션에서는 Windows 캡처 깜빡임, 혼합 DPI의 다중 모니터 동작, macOS 런타임 동작을 시각적으로 검증하지 못했습니다.
- updater artifact는 전용 Tauri minisign key로 서명합니다. Windows와 macOS 앱 package 자체는 code signing되지 않아 SmartScreen 또는 Gatekeeper 경고가 표시될 수 있습니다.

## [1.2.1](https://github.com/toughCSB/Ultra-Clipboard/compare/v1.2.0...v1.2.1) (2026-09-18)

### 추가

- 스크린샷을 클립보드에 복사하지 않고 편집기 창만 닫는 삭제 후 닫기 동작을 추가했습니다.

### 변경

- Windows 트레이 아이콘을 빨간 배경과 대비가 높은 흰색 클립보드 표시로 개선했습니다.
- 설정 사이드바의 앱 이름에 Pretendard Black을 적용했습니다.

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
