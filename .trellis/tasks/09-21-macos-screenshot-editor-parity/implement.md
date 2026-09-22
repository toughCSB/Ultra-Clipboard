# Implementation Plan

## 1. 작업 기반 준비

- [ ] Trellis task를 시작하고 `master` 기준 전용 작업 브랜치를 만든다.
- [ ] 사용자의 기존 untracked `tmp/bd1e0fbf-d8ad-4173-8f6b-eefeb18f6144.png`를 변경하거나 삭제하지 않는다.
- [x] Mac에 격리된 checkout을 준비하고 정식 앱 v1.2.2를 `/Applications/Ultra Clipboard.app`에 설치한다. 별도 QA 앱과 전용 데이터를 제거하고 QA bundle ID 권한 기록을 초기화한다.
- [ ] 작업 전 Windows와 Mac의 기본 lint/type/test/build 상태를 기록한다.

## 2. macOS 번들 및 의존성

- [ ] `objc2-screen-capture-kit`, `objc2-core-graphics`, `objc2-vision`, `block2`를 macOS target 의존성으로 추가하고 lockfile을 갱신한다.
- [ ] 실제 호출에 필요한 feature만 남기고 `cargo tree -d`를 확인한다.
- [ ] macOS 최소 버전을 `14.0`으로 설정한다.
- [ ] `NSScreenCaptureUsageDescription`과 한국어·영어 `InfoPlist.strings`를 번들에 추가한다.
- [ ] 생성된 `.app/Contents/Info.plist`와 localization 리소스를 실제 build artifact에서 확인한다.

## 3. 캡처 컨텍스트와 권한

- [ ] 플랫폼 backend에 `CaptureContext` 계약을 추가하고 Windows 구현을 기존 GDI 경로에 연결한다.
- [ ] macOS 화면 기록 권한 preflight/request와 사용자 오류 상태를 구현한다.
- [ ] `SCShareableContent` 비동기 조회, 제한 시간, 오류 변환을 구현한다.
- [ ] 캡처 실패나 제한 시간 초과 시 `ScreenshotState.capturing`이 항상 해제되는지 테스트한다.
- [ ] 권한 미허용 시 Preferences의 화면 기록 설정으로 이동하고 강조한다.

## 4. 디스플레이 매핑과 픽셀 캡처

- [ ] SCDisplay와 Tauri monitor를 안정적으로 매칭하는 display catalog를 구현한다.
- [ ] 물리 픽셀 rect와 display-local point sourceRect 사이의 변환을 구현한다.
- [ ] 1x/2x, 혼합 배율, 음수 원점, 상하 배치, 경계 교차 반례의 단위 테스트를 추가한다.
- [ ] `SCScreenshotManager`로 rect를 캡처하고 CGImage를 불투명 RGBA로 정규화한다.
- [ ] cursor 제외, SDR/sRGB, 출력 픽셀 크기 검증을 구현한다.
- [ ] 한 캡처에서 shareable content와 display catalog를 재사용한다.

## 5. 창 선택과 포커스

- [ ] Window Server 창을 앞에서 뒤 순서로 열거한다.
- [ ] 자체 프로세스, 바탕 화면 레이어, 비가시/투명/빈 창을 제외한다.
- [ ] 창 bounds를 공용 물리 픽셀 좌표로 변환하고 다중 모니터 교차를 테스트한다.
- [ ] 이전 frontmost application을 저장하고 취소 시 복원한다.

## 6. 화면 기록 권한 UI

- [ ] permission type에 `screenRecording`을 추가한다.
- [ ] Preferences permission section과 Onboarding에 화면 기록 항목을 추가한다.
- [ ] 기존 macOS permission API로 상태 확인, 요청, 폴링 복귀를 연결한다.
- [ ] 한국어·영어 설정 문구, 검색어, 아이콘을 추가하고 locale key 동기화를 확인한다.

## 7. macOS Vision OCR

- [ ] RGBA 입력 검증과 메모리 PNG 변환을 구현한다.
- [ ] `VNRecognizeTextRequest`의 accurate 모드, 자동 언어 감지와 교정을 구현한다.
- [ ] observation을 읽기 순서로 정렬하고 최상위 텍스트를 줄바꿈 문자열로 반환한다.
- [ ] OCR 버튼과 `⌘⇧O`를 macOS에 노출한다.
- [ ] 한국어, 영어, 혼합 언어, 작은 글자, 빈 이미지, 잘못된 입력을 검증한다.

## 8. 문서와 자동 검증

- [ ] README의 Windows 전용 문구를 제거하고 macOS 14 이상 지원과 화면 기록 권한을 설명한다.
- [x] `pnpm lint`, `pnpm tsc`, `pnpm test`, `pnpm build`를 실행한다.
- [x] Windows에서 `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets --all-features`를 실행한다.
- [ ] Windows x64 Tauri build와 캡처·편집·OCR 핵심 경로를 회귀 검증한다.
- [ ] Mac에서 같은 Rust/Frontend 검사와 Apple Silicon Tauri build를 실행한다.
- [x] macOS Intel target build를 실행하고 `x86_64` Mach-O 실행 파일을 확인했다. 릴리스 CI 빌드는 별도 검증이 필요하다.

## 9. 실제 Mac QA와 성능

- [ ] 정식 앱에서 화면 기록 권한 미결정, 허용, 거부/설정 이동 상태를 검증한다. 새 ad hoc 서명에 대해 사용자가 세 권한을 다시 허용했고, 앱 재시작 후 영역 캡처 성공을 확인했다. 미결정·거부 상태의 전체 경로는 미검증이다.
- [ ] 영역·창·전체 화면·지연·반복 캡처를 실제 화면에서 실행한다.
- [ ] 편집 도구, 실행 취소·다시 실행, 복사, 저장, 화면 고정, 드래그 출력, OCR을 확인한다.
- [ ] 단일 4K Retina에서 선택 좌표와 출력 픽셀을 확인한다.
- [ ] MacBook 내장 화면을 활성화해 혼합 배율 다중 모니터를 확인한다. 사용할 수 없으면 자동 테스트 통과와 실제 미검증 상태를 명시해 release gate를 유지한다.
- [ ] 영역 10회, 전체 화면 10회, 창·반복·지연 각 5회의 성공률과 중앙값/p95를 기록한다.
- [ ] 검은 화면, stale frame, 오버레이 포함, 깜박임 또는 멈춤이 없는지 Orca 화면 캡처로 확인한다.
- [x] 사용자가 정식 Mac 앱에서 영역 캡처를 반복한 뒤 캡처 시작과 편집기 인계의 깜빡임이 사라졌다고 확인했다.
- [ ] 새 성능 빌드에서 오버레이·편집기 표시 시간과 깜빡임 부재를 반복 측정하고 실제 화면에서 확인한다. 영역 캡처 3회는 기록했고 사용자가 깜빡임 제거를 확인했다. 전체 acceptance 횟수와 p95 측정은 남아 있다.

## 10. 완료와 릴리스 준비

- [ ] 요구사항과 acceptance criteria를 대조하고 Trellis quality check를 수행한다.
- [x] 다음 릴리스 후보 v1.2.3의 한국어·영어 변경 기록 초안을 `release-draft.md`에 준비한다. 버전 확정과 정식 changelog 반영은 릴리스 gate 통과 후 수행한다.
- [x] 관련 변경만 한국어 Conventional Commit `80a6096`으로 커밋하고 `master`에 fast-forward 병합했다.
- [x] `master`를 `origin/master`에 푸시하고 원격 커밋이 일치하는지 확인했다.
- [ ] tag, GitHub release 및 updater 배포는 남은 실제 기기 검증과 unsigned macOS 업데이트의 권한 재허용 안내를 확인한 뒤 진행한다. Apple Developer 계정이 없으므로 기존 v1.2.2와 같은 배포 정책을 유지한다.

## 11. 공통 편집기 개선

- [x] CleanShot X, ShareX, Acrobat, OneNote의 공식 설명에서 형광펜 조절 방식을 조사한다.
- [x] 형광펜 색·진하기를 도구 기본값과 개별 표시 모두에서 편집할 수 있게 한다.
- [x] 밝고 어두운 캡처의 하이라이트 합성을 개선한다.
- [x] 캡처 버리기 버튼을 작은 빨간 아이콘 전용 버튼으로, 주 복사 버튼을 더 크게 바꾼다.
- [x] Mac·Windows에서 새 아이콘 배치와 화면별 지연 로딩, 선택 오버레이의 직접 캔버스 그리기를 실제 사용 경로로 확인한다. Mac은 사용자가 영역 캡처 3회와 편집기 표시·깜빡임 부재·휴지통 배치를 확인했고, Windows는 설치본의 영역·전체 화면 캡처와 편집기 배치를 Orca 화면으로 확인했다.
- [x] Windows 설치본에서 형광펜 표시를 그린 뒤 선택 표시의 진하기 슬라이더가 75%에서 98%로 바뀌는 것을 확인했다. 테스트 캡처는 휴지통으로 버렸다.
- [x] Mac 설치본에서 형광펜 진하기와 선택 표시 재편집을 실제 화면으로 확인한다.

## 2026-09-22 검증 기록

- macOS 정식 앱의 새 임시 서명에 대해 사용자가 화면 기록·손쉬운 사용·전체 디스크 접근을 다시 허용했다. 앱 재시작 후 영역 캡처 3회가 성공했고 사용자는 깜빡임 제거와 휴지통 배치를 확인했다. 약간의 체감 지연은 남는다.
- Mac 영역 캡처 3회 중 warm 2회의 두 화면 동결은 143ms·162ms, 오버레이 창 준비는 1ms·0ms였다. 큰 화면의 프레임 수신은 99ms·140ms, 캔버스 그리기는 18ms·20ms였다. 선택 후 편집기 창 준비는 78ms·65ms, 새 WebView의 첫 그리기는 386ms·380ms였다. 전체 사용자 체감 시간의 p95는 아직 측정하지 않았다.
- Windows 정식 설치 경로에 1.2.2 검증 빌드를 설치했고 기존 `prod` 클립보드 DB가 남아 있다. 영역 캡처의 동결은 97ms, 전체 화면 캡처의 픽셀 획득은 91ms였다. 편집기에서 휴지통 아이콘 전용 버튼과 더 큰 주 버튼을 시각 확인했고, 형광펜을 그리고 진하기 슬라이더를 조절했다.
- Windows 표준 NSIS 빌드는 설치 파일을 생성했으나 로컬 updater 개인 키가 없어 명령의 마지막 서명 단계가 실패했다. 저장소 배포 설정은 바꾸지 않고, updater 산출물을 끄고 현재 사용자 설치 모드로 바꾼 일회성 로컬 빌드 설정으로 검증용 NSIS 빌드·설치를 완료했다. 공식 릴리스 서명과 updater 산출물 검증은 남아 있다.
- Windows에서 `pnpm lint`, `pnpm tsc`, `pnpm test`(47개), `pnpm build`, `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets --all-features`(230개)가 통과했다. Mac Apple Silicon 빌드와 `cargo test --all-targets --all-features`(228개)도 통과했다.
- 최종 리뷰에서 Mac 정식 앱의 전체 화면 캡처 6720×3780과 창 캡처 998×844를 실제 편집기에서 확인했다. 테스트용 TextEdit 창의 `ULTRA CLIPBOARD QA 123` 문구를 Vision OCR이 클립보드에 복사했고, 이미지 복사 후 `public.png`·`public.tiff` 클립보드 형식을 확인했다.
- Mac에는 동일 배율 2x 화면 두 대가 활성화되어 있어 혼합 배율 실제 QA는 아직 남아 있다. 격리된 Rust 1.96.0 도구체인으로 macOS Intel `cargo check`와 `cargo build`가 통과했고 생성된 실행 파일이 `x86_64` Mach-O임을 확인했다. 현재 Mac 설치본은 ad hoc 서명이며 `security find-identity -p codesigning -v`에서 유효한 배포 서명 identity가 0개로 확인됐다. 릴리스 CI에도 Apple Developer 서명 설정이 없어 새 버전의 TCC 권한 유지가 검증되지 않았다.
- Mac 형광펜 버튼을 선택하고 macOS `CGEvent` 드래그로 실제 이미지를 그렸다. 표시를 선택한 뒤 진하기를 74%에서 93%로 바꾸자 캔버스 픽셀이 달라졌고, 휴지통 버튼으로 해당 캡처를 버렸다. Orca의 단순 드래그는 캔버스 입력으로 전달되지 않아 이 검증에는 사용하지 않았다.
- Mac 정식 앱에서 테스트용 TextEdit 문서만 영역 캡처해 편집기의 저장 버튼을 눌렀다. macOS 저장 시트에서 `/tmp/ultra-clipboard-output-qa.png`를 선택했고, 생성된 파일은 실제 790×330 RGBA PNG였다. 편집기 드래그 버튼에서 TextEdit으로 놓자 캐시된 790×330 PNG의 파일 경로가 대상 문서에 입력됐다. 테스트 문서는 실행 취소로 원래 내용으로 되돌렸다. TextEdit의 일반 텍스트 문서에는 이미지가 삽입되지 않으므로 이미지 수용 앱으로의 드롭과 편집 도구 전체의 Mac 실기 검증은 남아 있다.
- Mac 마지막 영역 반복 캡처는 998×844 이미지를 직접 편집기에 열었고, 화면 고정은 같은 크기의 `Ultra Clipboard Pin` 창 생성까지 확인했다. 지연 캡처는 3초 후 영역 선택 오버레이가 표시됐지만 원격 드래그 입력이 선택을 확정하지 못해 편집 결과는 미검증이다.
- Mac 정식 앱 설정에 손쉬운 사용과 전체 디스크 접근이 꺼짐으로 표시된 시점에 macOS 시스템 설정을 직접 확인하니 두 권한도 실제로 꺼져 있었다. 두 항목을 다시 켜고 앱을 재시작한 뒤 앱 설정에서 화면 기록·손쉬운 사용·전체 디스크 접근이 모두 켜짐으로 표시되는 것을 확인했다. ad hoc 서명이 바뀌면 권한 재허용이 필요한 문제는 배포 서명 identity 확보 전까지 남는다.
- GitHub Actions의 등록된 secret은 Tauri updater 서명 키 두 개뿐이며 Apple Developer 배포 서명·notarization 자격증명은 없다. Mac의 Homebrew Rust에는 `cargo-clippy`가 없어 격리된 Rust 1.96.0 도구체인으로 macOS `cargo clippy --all-targets --all-features --offline -- -D warnings`를 통과했다. Mac Rust 테스트와 Apple Silicon 빌드는 앞서 통과했다.
- 최종 재점검에서 Mac Dock 재열기 시 `window not found: preference`가 기록됐다. 설정 WebView가 유휴 정리된 후 `macos::handle_reopen`이 다시 생성하지 않는 플랫폼 표시 함수를 직접 호출한 것이 원인이었다. 공통 `window::show_window`로 변경하고 Mac clippy를 통과했다. 새 Apple Silicon `.app`의 실행 파일과 번들을 생성했으나 로컬 Tauri 명령은 updater 개인 키가 없어 마지막 서명 단계에서 종료됐다. 검증용 임시 서명 후 설치해 설정 창을 닫고 70초 이상 지난 뒤 Dock 재열기로 새 설정 창이 생성되는 것을 확인했다.
- 검증용 앱의 새 ad hoc 서명으로 Mac 앱 설정의 화면 기록·손쉬운 사용 권한은 다시 꺼짐으로 표시됐다. 기존 설치본을 복구하고 앱을 재시작했으며, 화면 기록 권한 확인과 영역 캡처 오버레이 표시가 성공했다. 임시 서명 앱을 새 릴리스로 배포하지 않는다.
- `v1.2.3` 배포 후보 `753087e`를 `master`에 푸시하고 태그를 생성했다. [Release CI run 35689588299](https://github.com/toughCSB/Ultra-Clipboard/actions/runs/35689588299)의 Windows x64/ARM64 및 macOS Apple Silicon/Intel 빌드가 모두 통과했고, GitHub 초안 릴리스에 11개 파일이 생성됐다. `latest.json`의 8개 플랫폼 URL이 해당 asset을 가리키며 서명 문자열이 각 `.sig`와 일치한다. 암호학적 서명 검증은 아직 수행하지 않았다.
- CI의 Apple Silicon `.app.tar.gz`를 실제 Mac에서 설치·실행했고, 앱 설정에서 `v1.2.3`과 기존 로컬 저장량 41 MB를 확인했다. 번들의 `Info.plist`는 최소 macOS 14.0이고 실행 파일은 arm64다. 번들 전체는 유효한 배포 서명이 없어 `codesign --verify --deep --strict`가 실패했으며, 실행에 성공한 뒤 앱 설정의 화면 기록·손쉬운 사용·전체 디스크 접근 세 권한이 모두 꺼짐으로 표시됐다. 기존 설치본을 복구하고 코드 서명 검사와 프로세스 재실행을 확인했다. 초안 릴리스는 아직 공개하지 않았다.
- 복구한 정식 Mac 앱 v1.2.2를 재시작한 뒤 macOS 시스템 설정에서 Ultra Clipboard의 전체 디스크 접근 스위치가 켜진 화면을 확인했지만 앱 설정에는 꺼짐으로 표시됐다. 같은 화면에서 화면 기록·손쉬운 사용은 켜짐이었다. 사용 중인 `tauri-plugin-macos-permissions` 2.3.0의 전체 디스크 접근 확인은 Safari·Stocks 보호 폴더의 `read_dir` 성공 여부만 확인하므로 시스템 설정 스위치의 대리 지표가 아니다. 실제 특정 보호 파일 접근 가능 여부는 아직 검증하지 않았다.
- 전체 디스크 접근 설정을 확정적인 스위치 대신 시스템 설정으로 이동하는 버튼으로 바꾸고 두 언어의 설명을 수정했다. Windows에서 `pnpm lint`, `pnpm tsc`, `pnpm test`(47개), `pnpm build`가 통과했다. 기존 Mac 앱의 권한을 또 무효화하지 않도록 새 앱 설치는 보류했고, 새 화면의 Mac 실기 확인과 보류 중인 v1.2.3 릴리스 정리는 남아 있다.
- 실제 Mac 설치본 v1.2.2의 `codesign -dv -r-`에서 `Signature=adhoc`, `TeamIdentifier=not set`, `designated => cdhash H"d9287df9ee84662596e35bb9a349ce4e0ab243db"`를 확인했다. `security find-identity -p codesigning -v`에는 유효한 인증서가 0개다. 이 서명 형태는 버전이 바뀌면 권한 식별이 유지되지 않는다는 Apple TN3127의 설명과 일치한다. 현재 설치본은 교체하지 않았다.
- 전체 디스크 접근은 앱 코드에서 권한 검사와 설정 링크 외에 직접 요구하는 핵심 경로가 없고, 시작 안내가 필수라고 잘못 설명하고 있었다. 시작 권한 카드에서 제외하고 설정에는 선택 권한으로 설명했다. 두 언어와 README를 동기화했으며 Windows에서 `pnpm lint`, `pnpm tsc`, `pnpm test`(47개), `pnpm build`가 통과했다. 화면 기록·손쉬운 사용의 업데이트 후 권한 유지 문제는 서명 identity를 고정해 실제 두 버전 업데이트로 검증하기 전까지 미해결이다.
- 사용자가 최종 설치·배포까지 진행하라고 명시했다. 공개되지 않은 v1.2.3 태그는 그대로 두고 최신 수정이 포함된 v1.2.4를 첫 공개 업데이트로 준비한다. CI의 v1.2.3 Apple Silicon updater archive를 풀어 보니 번들은 `code has no resources but signature indicates they must be present`로 strict 검증에 실패했고 실행 파일만 linker-signed 상태였다. 복사본을 `codesign --force --deep --sign -`로 서명한 뒤 strict 검증과 정식 bundle ID가 통과했다. 사용자가 Mac 릴리스 설정의 `signingIdentity: "-"` 변경을 승인했고, 다음 CI 산출물의 실제 `.app` 서명을 다시 검증한다.
- `v1.2.4`를 `master` 커밋 `6bace55`와 태그로 푸시했다. [Release CI run 35696489614](https://github.com/toughCSB/Ultra-Clipboard/actions/runs/35696489614)의 Windows x64/ARM64 및 macOS Apple Silicon/Intel 빌드가 모두 통과했고, 초안 릴리스에 11개 asset이 있다. `latest.json` 8개 플랫폼 URL이 해당 asset을 가리키고 서명 문자열이 각 `.sig`와 일치한다.
- CI의 Apple Silicon 및 Intel `.app.tar.gz`를 Mac에서 풀어 두 번들 모두 `codesign --verify --deep --strict`를 통과했다. Apple Silicon 정식 앱을 `/Applications/Ultra Clipboard.app`에 v1.2.4로 교체했고 기존 사용자 데이터 41 MB, 클립보드 기록 126건 및 SQLite `quick_check=ok`를 확인했다. 이전 v1.2.2 앱과 사용자 데이터는 Mac의 `/tmp`에 복구용으로 보관했다.
- Windows 정식 x64 NSIS 설치 파일을 현재 사용자 방식으로 설치하고 v1.2.4 앱을 재실행했다. 실제 영역 캡처로 계산기 330×480 이미지를 편집기에 열어 휴지통 아이콘과 큰 복사 버튼을 화면 확인한 후 테스트 캡처를 버렸다. 기존 기록 248건과 SQLite `quick_check=ok`를 확인했다.
- 새 Mac 앱의 화면 기록·손쉬운 사용 체크가 꺼져 있었고 실제 영역 캡처도 `screen recording permission is required`로 실패했다. 이전 v1.2.2 임시 서명의 지정 요구사항은 `cdhash d9287d...`, 새 v1.2.4는 `cdhash e8b282...`로 다르다. 사용자가 macOS 시스템 설정의 두 스위치를 껐다 켜고 앱을 재시작해도 실패했다. 사용자 승인 후 정식 bundle ID의 `ScreenCapture`·`Accessibility` TCC 기록만 초기화하고 다시 허용하자 앱 체크가 둘 다 켜졌다. 실제 영역 캡처는 두 모니터의 프레임을 528ms에 동결하고 1698×1263 선택 이미지를 편집기에 열어 첫 그리기를 완료했다. 임시 서명으로 인한 버전 간 권한 유지 문제는 여전히 남는다.
- GitHub 릴리스 `v1.2.4`를 2026-09-22 07:18:47 UTC에 공개했다. [공개 릴리스](https://github.com/toughCSB/Ultra-Clipboard/releases/tag/v1.2.4)의 11개 asset과 `latest.json`을 확인했고, 공개 `latest.json`의 SHA-256은 검증한 초안 파일과 같다. Windows x64 및 macOS Apple Silicon updater asset URL은 인증 없이 `200 application/octet-stream`으로 전체 파일을 제공했다.
- [ ] 실제 픽셀 결과와 편집기 배치를 Windows·macOS에서 검증한다.

## 품질 Gate

- Mac 실제 캡처·편집·출력 경로가 성공하지 않으면 완료로 처리하지 않는다.
- Apple Silicon 실행 검증과 Intel 빌드 검증이 모두 없으면 릴리스 대상으로 처리하지 않는다.
- Windows 자동 검사와 핵심 캡처 회귀 검증이 통과하지 않으면 병합·릴리스하지 않는다.
- 현재 실제 Mac에 두 번째 활성 화면이 없으므로 혼합 배율 hardware QA는 내장 화면 활성화 전까지 미검증으로 표시한다.
