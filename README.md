<div align="center">
  <img src="./public/logo.png" alt="Ultra Clipboard" width="96" height="96" />

# Ultra Clipboard

**macOS와 Windows를 위한 로컬 우선 클립보드 관리자**

한국어 | [English](./README.en.md)

  <br />

  <img alt="Tauri v2" src="https://img.shields.io/badge/Tauri-v2-24c8db?style=flat-square" />
  <img alt="Rust first" src="https://img.shields.io/badge/Rust-first-b7410e?style=flat-square" />
  <img alt="React 19" src="https://img.shields.io/badge/React-19-61dafb?style=flat-square" />
  <img alt="macOS" src="https://img.shields.io/badge/macOS-supported-000000?style=flat-square&logo=apple&logoColor=white" />
  <img alt="Windows" src="https://img.shields.io/badge/Windows-supported-0078d4?style=flat-square&logo=windows&logoColor=white" />
</div>

## 소개

Ultra Clipboard는 복사한 내용을 기기에 저장하고 빠르게 다시 찾는 오픈 소스 데스크톱 클립보드 관리자입니다. Rust 중심의 Tauri 구조와 React UI로 구성되어 있으며, 한국어와 영어 UI를 지원하고 기본 언어는 한국어입니다.

이 프로젝트는 [EcoPasteHub의 EcoPaste](https://github.com/EcoPasteHub/EcoPaste)를 기반으로 한 독립 유지보수 fork입니다. EcoPasteHub와 공식적으로 제휴하거나 소속된 프로젝트가 아니며, Ultra Clipboard의 개발과 배포는 `toughCSB`가 독립적으로 관리합니다.

## 주요 기능

- 일반 텍스트, HTML, RTF, 이미지, 파일과 폴더를 클립보드 기록으로 저장
- 본문과 메모를 SQLite FTS5로 검색
- 원본 애플리케이션과 콘텐츠 유형으로 기록 필터링
- 파일 기록의 전체 경로 표시와 파일 위치 열기
- 텍스트, 이미지, 파일 미리보기
- 붙여넣기, 복사, 일반 텍스트로 복사, 링크 열기, 메모, 즐겨찾기, 고정, 삭제
- 즐겨찾기, 고정 항목, 메모와 사용자 지정 그룹으로 기록 정리
- 복사 완료 사운드 설정
- 민감한 값으로 판단되는 private key, service token, AWS key, JWT 등의 수집 및 표시 보호
- 기록 기본 보존 기간 1개월, 약 30일. 보존 기간은 설정에서 변경할 수 있으며 즐겨찾기와 고정 항목은 자동 정리 대상에서 제외
- 데이터, 리소스와 설정을 로컬에 저장하는 local-first 방식
- 명시적으로 등록한 Tailscale 기기 사이의 텍스트·서식·이미지 실시간 공유
- 어두운 작업표시줄에서도 또렷하게 보이는 시스템 트레이 아이콘

## 스크린샷 편집기

화면 캡처와 편집기를 내장하고 있습니다. Windows와 macOS 14 이상에서 영역·창·전체 화면·지연·반복 캡처와 편집, OCR, 복사, 저장, 화면 고정, 드래그 출력을 사용할 수 있습니다. macOS에서는 처음 캡처할 때 화면 기록 권한이 필요하며, 혼합 배율 모니터의 실제 동작은 아직 검증되지 않았습니다.

- 화살표, 텍스트, 자, 사각형, 배경, 펜, 돋보기, 흐리게, 형광펜, 스포트라이트, 번호, 타원, 선 등 13종 주석 도구
- 도구마다 구분되는 색상 아이콘과 드래그로 재배치 가능한 툴바
- 도구는 한 번 클릭하면 기존 설정으로 즉시 적용되고, 더블클릭하면 해당 아이콘 바로 아래에 반투명 옵션 패널이 열림
- 아이콘에 마우스를 올리면 어떤 도구인지 툴팁으로 안내
- 캡처 영역의 텍스트를 인식해 복사하는 OCR
- 반복 캡처와 지연 캡처
- 클립보드의 이미지를 편집기로 바로 붙여넣기
- 클립보드에 복사하는 동시에 창을 닫는 원클릭 버튼 (Ctrl/⌘+Enter 단축키 지원)

## 백업과 데이터 호환성

Ultra Clipboard는 `.ecopastebak` 백업을 내보내고 가져올 수 있습니다. 암호화된 `.ecopastebak` 백업도 지원하며, 백업에는 기록 데이터와 리소스, 설정이 포함됩니다. 백업을 병합하거나 기존 데이터를 덮어쓰는 방식으로 복원할 수 있습니다.

WebDAV 백업과 복원은 설정에서 직접 실행하는 수동 기능입니다. 별도로 Tailscale IP와 peer 공유 키를 설정하면 텍스트, HTML, RTF와 PNG 기록을 실시간 공유할 수 있습니다. 파일과 설정은 동기화하지 않으며 WebDAV 비밀번호와 Tailscale peer 설정은 백업 파일에 포함하지 않습니다.

Ultra Clipboard는 EcoPaste와 별도의 데이터 namespace를 사용합니다. 기존 공식 EcoPaste 데이터를 사용하려면 EcoPaste에서 `.ecopastebak` 백업을 만든 뒤 Ultra Clipboard의 백업 가져오기로 가져오세요. 기존 데이터를 자동으로 이전하지 않습니다.

## 지원 운영체제

- Windows
- macOS 14 이상

Linux는 지원하지 않습니다. macOS Apple Silicon과 Intel 패키지는 빌드 대상으로 설정되어 있습니다.

## 다운로드 및 설치

최신 빌드는 [Releases](https://github.com/toughCSB/Ultra-Clipboard/releases)에서 받을 수 있습니다.

앱 package 자체는 code signing과 공증(notarization)이 되어 있지 않습니다(updater artifact만 전용 Tauri key로 서명). Windows SmartScreen 또는 macOS Gatekeeper가 경고를 표시할 수 있으므로 출처와 파일을 확인한 뒤 사용하세요.

macOS에서는 앱 업데이트로 임시 서명이 바뀌면 화면 기록·손쉬운 사용·전체 디스크 접근 권한을 다시 허용해야 할 수 있습니다. 권한이 꺼진 것으로 표시되면 macOS 시스템 설정에서 `Ultra Clipboard`의 해당 항목을 확인하고 앱을 다시 시작하세요. 기존 클립보드 기록과 앱 설정은 업데이트로 삭제되지 않습니다.

### macOS에서 "손상되었기 때문에 열 수 없습니다" 오류

macOS는 Apple Developer 인증서로 서명되지 않은 앱을 인터넷에서 받으면 격리(quarantine) 속성을 붙이고 Gatekeeper가 실행을 막습니다. 최신 macOS(Sonoma/Sequoia 이상)에서는 서명이 전혀 없는 앱에 한해 "확인되지 않은 개발자" 대신 더 강한 "손상되었기 때문에 열 수 없습니다" 문구가 뜹니다. **앱이 실제로 손상된 것은 아닙니다.**

`.app`을 `/Applications`로 옮긴 뒤, 터미널에서 아래 명령을 한 번 실행하면 정상적으로 열립니다.

```bash
xattr -cr /Applications/Ultra\ Clipboard.app
```

`Operation not permitted` 오류가 뜨면, 최신 macOS는 터미널이 다른 앱의 속성을 건드리려면 별도 권한이 필요합니다:

1. **시스템 설정 → 개인정보 보호 및 보안 → App 관리(App Management)** 로 이동
2. 사용 중인 터미널 앱(Terminal.app, iTerm2 등)을 목록에서 켜기 (없으면 `+`로 추가)
3. 터미널 앱을 완전히 종료 후 재실행하고 위 명령을 다시 실행

**근본적으로 이 경고 자체를 없애려면** Apple Developer Program(유료, 연 $99) 가입 후 코드사이닝과 공증을 거쳐야 합니다. 인증서·계정 발급이 필요한 별도 작업이라 아직 적용하지 못했고, 현재로서는 위 우회 방법이 유일한 해결책입니다.

## 개발

필요한 환경은 macOS 또는 Windows, Node.js 20 이상, pnpm 10 이상, Rust toolchain입니다.

```bash
pnpm install
pnpm tauri dev
pnpm tauri build
```

프론트엔드 검사와 Rust 검사는 다음처럼 실행합니다.

```bash
pnpm lint
pnpm tsc
cd src-tauri
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

자세한 개발 규칙과 구조는 [기여 안내](./CONTRIBUTING.md)를 참고하세요.

## 라이선스

Ultra Clipboard는 [Apache License 2.0](./LICENSE)으로 배포됩니다. 원 프로젝트의 저작권 고지는 유지되며, fork의 수정 사항은 [NOTICE](./NOTICE)에 별도로 기록되어 있습니다.
