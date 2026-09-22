# Research

## 확인한 현재 상태

- 기준 브랜치는 `master`, 기준 커밋은 `b2f1fe771700cb5aecb4ce322ea1a01dd67c2c04`이며 조사 시점에 `origin/master`와 일치한다.
- macOS 캡처 백엔드 `src-tauri/src/screenshot/backend/macos.rs`는 `is_supported() == false`인 명시적 스텁이다.
- Windows 백엔드는 GDI 화면 복사, 앞에서 뒤 순서의 창 목록, 이전 활성 창 복원을 구현한다.
- 영역·창·전체 화면·지연·반복 모드, 동결 오버레이, 편집기, 화면 고정, 복사·저장·드래그 출력은 공용 Rust/React 경로를 사용한다.
- OCR은 `src-tauri/src/screenshot/ocr.rs`의 Windows WinRT 구현만 존재하며 macOS에서는 오류를 반환한다. 편집기 UI도 OCR 버튼과 `Shift+O`를 Windows에만 노출한다.
- 현재 macOS 권한 UI에는 손쉬운 사용과 전체 디스크 접근만 있고 화면 기록 항목은 없다. 설치된 권한 플러그인은 화면 기록 확인·요청 API를 이미 제공한다.
- 릴리스 CI는 Apple Silicon, Intel, Windows x64, Windows ARM64 산출물을 만들며 PR CI는 macOS와 Windows에서 Rust 검사를 실행한다.
- macOS 설정에는 최소 지원 버전과 `NSScreenCaptureUsageDescription`이 없다.
- README는 캡처·편집 기능을 Windows 전용으로 안내하고 있다.

## 실제 Mac 환경

- SSH 별칭 `mac`으로 접속된다.
- macOS `26.6.2`, Apple Silicon `arm64`, Xcode `26.6`, Rust `1.94.0`, Node.js `22.23.1`, Orca `1.4.205`가 확인됐다.
- 로그인 셸에서 Rust와 Node 경로가 준비된다. 전역 `pnpm`은 없지만 저장소는 `npx pnpm@10.33.1` 실행 경로를 이미 사용한다.
- 현재 활성 디스플레이는 LG HDR 4K 한 대이며 논리 해상도 `3360x1890`, 픽셀 해상도 `6720x3780`이다. 실제 혼합 배율 다중 모니터 검증에는 MacBook 내장 화면을 함께 활성화하거나 두 번째 디스플레이가 필요하다.
- Mac의 Orca 데스크톱 런타임은 동작한다. 저장소는 아직 Mac에 등록되거나 체크아웃되지 않았다.

## 기술 선택 근거

### 화면 캡처

- 최소 지원 버전을 macOS 14로 정했으므로 `SCScreenshotManager`의 일회성 비동기 캡처 API를 사용할 수 있다.
- `SCShareableContent`를 캡처 한 번당 한 차례 조회하고 `SCDisplay`와 앱/창 메타데이터를 재사용한다.
- `SCContentFilter`와 `SCStreamConfiguration`으로 대상 디스플레이의 부분 영역을 지정하고 출력 크기를 물리 픽셀과 일치시킨다.
- 커서는 Windows GDI 경로와 맞추기 위해 캡처하지 않는다. 결과는 기존 편집기가 요구하는 불투명 8비트 RGBA/sRGB로 정규화한다.
- 필요한 Rust 바인딩은 `objc2-screen-capture-kit`, 콜백 블록을 직접 만들기 위한 `block2`, 반환 `CGImage` 처리를 위한 `objc2-core-graphics`다.

### 화면 기록 권한

- Rust 캡처 경로에서 `CGPreflightScreenCaptureAccess`와 `CGRequestScreenCaptureAccess`를 사용해 단축키·트레이 시작도 같은 정책을 적용한다.
- 설정과 온보딩 UI에서는 기존 `tauri-plugin-macos-permissions-api`의 화면 기록 확인·요청 API를 사용한다.
- 최초 요청 또는 거부 후에도 권한이 없으면 환경설정의 화면 기록 항목을 열고 해당 설정을 강조한다.
- 시스템 권한 설명은 `Info.plist`와 한국어·영어 `InfoPlist.strings`에 제공한다.

### 좌표와 창 선택

- Tauri의 오버레이와 상태는 가상 데스크톱 물리 픽셀을 계속 기준으로 사용한다.
- macOS 백엔드는 `SCDisplay.displayID`, 디스플레이 프레임, 픽셀 크기, Tauri 모니터 bounds와 scale factor를 한 번 매칭한다.
- 대상 사각형을 디스플레이 로컬 point 단위 `sourceRect`로 변환하고 출력 `width`/`height`는 원래 물리 픽셀 크기로 고정한다.
- 창 후보는 Window Server 목록에서 화면 표시 여부, 레이어, alpha, 소유 PID, 크기를 검사하고 앞에서 뒤 순서를 유지한다. 좌표 변환은 캡처와 같은 디스플레이 매핑을 사용한다.
- 변환 로직은 순수 함수로 분리해 Retina, 비 Retina, 음수 원점, 세로 배치, 동일 크기 디스플레이 반례를 단위 테스트한다.

### OCR

- macOS Vision의 `VNRecognizeTextRequest`와 `VNImageRequestHandler`를 사용한다.
- 인식 수준은 `accurate`, 자동 언어 감지와 언어 교정을 활성화한다.
- 기존 RGBA를 메모리 PNG로 인코딩해 Vision에 전달하면 픽셀 포맷과 방향 차이를 줄일 수 있다.
- `VNRecognizedTextObservation`의 bounding box를 위에서 아래, 왼쪽에서 오른쪽 순으로 정렬하고 빈 결과를 제거해 Windows와 같은 줄 단위 문자열 계약을 유지한다.
- 필요한 Rust 바인딩은 `objc2-vision`이다. OCR은 온디바이스에서 처리하고 이미지를 외부 서비스로 보내지 않는다.

## 참고 자료

- Apple ScreenCaptureKit: <https://developer.apple.com/documentation/screencapturekit>
- Apple WWDC23, Take ScreenCaptureKit to the next level: <https://developer.apple.com/videos/play/wwdc2023/10136/>
- Apple Vision text recognition: <https://developer.apple.com/documentation/vision/recognizing-text-in-images>
- Tauri macOS application bundle: <https://v2.tauri.app/distribute/macos-application-bundle/>
- objc2 ScreenCaptureKit bindings: <https://docs.rs/objc2-screen-capture-kit/0.3.2/objc2_screen_capture_kit/>
- objc2 Vision bindings: <https://docs.rs/objc2-vision/0.3.2/objc2_vision/>
