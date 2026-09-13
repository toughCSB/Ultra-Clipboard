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

## 백업과 데이터 호환성

Ultra Clipboard는 `.ecopastebak` 백업을 내보내고 가져올 수 있습니다. 암호화된 `.ecopastebak` 백업도 지원하며, 백업에는 기록 데이터와 리소스, 설정이 포함됩니다. 백업을 병합하거나 기존 데이터를 덮어쓰는 방식으로 복원할 수 있습니다.

WebDAV 백업과 복원은 설정에서 직접 실행하는 수동 기능입니다. 별도로 Tailscale IP와 peer 공유 키를 설정하면 텍스트, HTML, RTF와 PNG 기록을 실시간 공유할 수 있습니다. 파일과 설정은 동기화하지 않으며 WebDAV 비밀번호와 Tailscale peer 설정은 백업 파일에 포함하지 않습니다.

Ultra Clipboard는 EcoPaste와 별도의 데이터 namespace를 사용합니다. 기존 공식 EcoPaste 데이터를 사용하려면 EcoPaste에서 `.ecopastebak` 백업을 만든 뒤 Ultra Clipboard의 백업 가져오기로 가져오세요. 기존 데이터를 자동으로 이전하지 않습니다.

## 지원 운영체제

- Windows
- macOS

Linux는 지원하지 않습니다. `v1.1.3`은 Windows와 macOS용 안정 릴리스입니다. macOS 패키지는 CI에서 빌드되지만 실제 macOS runtime QA는 아직 완료되지 않았습니다.

## 다운로드 및 설치

최신 빌드는 [Releases](https://github.com/toughCSB/Ultra-Clipboard/releases)에서 받을 수 있습니다.

`v1.1.3`의 updater artifact는 전용 Tauri key로 서명하지만 앱 package 자체는 code signing되지 않았습니다. Windows SmartScreen 또는 macOS Gatekeeper가 경고를 표시할 수 있으므로 출처와 파일을 확인한 뒤 사용하세요.

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
