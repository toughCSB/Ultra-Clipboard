# Design

## 범위 결정

한 번의 캡처 흐름이 권한, 디스플레이 매핑, 픽셀 획득, 창 후보, 오버레이, 편집기와 OCR을 연속해서 통과하므로 하나의 작업으로 구현한다. 기존 공용 오버레이·편집기·출력 계약은 유지하고 macOS 고유 시스템 기능을 Rust 백엔드에 추가한다.

## 공통 형광펜·삭제 버튼 개선

공식 제품 자료에서 CleanShot X는 하이라이트의 투명도와 둥근 모서리를 제공하고, ShareX는 원본을 가리지 않는 반투명 강조 영역을 설명한다. Acrobat은 하이라이트 색·투명도를, OneNote는 색·두께를 설정할 수 있다. 현재 편집기의 직사각형 드래그 방식은 유지하고 색상·진하기 조절을 추가한다.

- `ToolStyle.highlightOpacity`는 20–100 정수 백분율이며 기본값은 75다. 기존 `screenshot-editor:tool-style` localStorage에 필드가 없으면 기본값을 사용하고, 잘못된 값은 유효 범위로 제한한다.
- 새 `HighlighterShape.opacity`는 그릴 때의 진하기를 보존한다. 이미 그린 표시를 선택하면 그 표시의 색·진하기를 패널에 보여주고 수정한다.
- 둥근 영역에 얇은 `source-over` 잉크 층과 `multiply` 색 층을 순서대로 그려 어두운 캡처의 가시성과 밝은 캡처의 글자 대비를 함께 확보한다. 기존 `drawDocument`를 미리보기·출력이 공유한다.
- 캡처를 버리는 버튼은 빨간 아이콘 전용 버튼으로 표시한다. 휴지통 그림은 크게 유지하고 버튼 폭은 줄이며 접근성 이름과 툴팁은 유지한다. 주 작업인 복사 후 닫기 버튼은 더 크게 표시한다. 동작은 기존 `onDiscardAndClose` 경로를 유지한다.

참고: <https://cleanshot.com/changelog>, <https://getsharex.com/docs/image-editor>, <https://helpx.adobe.com/in_hi/acrobat/how-to/gather-feedback-pdf-comments.html>, <https://support.microsoft.com/en-us/onenote/onenote-help-and-learning/learn-more-about-drawing-tools>.

## 의존성과 배포 기준

macOS 대상 의존성에 다음 objc2 계열 crate를 추가한다. 구현 착수 승인은 이 외부 의존성 추가 승인도 포함한다.

- `objc2-screen-capture-kit = "0.3"`: macOS 14의 `SCShareableContent`, `SCContentFilter`, `SCStreamConfiguration`, `SCScreenshotManager`
- `objc2-core-graphics = "0.3"`: ScreenCaptureKit이 반환한 `CGImage` 처리
- `objc2-vision = "0.3"`: `VNRecognizeTextRequest`와 `VNImageRequestHandler`
- `block2 = "0.6"`: Objective-C 비동기 completion handler를 Rust 채널로 연결

기존 `core-graphics = "0.25"`, `objc2`, `objc2-foundation`, `objc2-app-kit`, `image`는 유지한다. 실제 호출에 필요한 최소 feature만 활성화하고 `cargo tree -d`로 예상하지 않은 중복을 확인한다.

`src-tauri/tauri.macos.conf.json`에 `bundle.macOS.minimumSystemVersion = "14.0"`을 지정한다. `src-tauri/Info.plist`에 `NSScreenCaptureUsageDescription`을 추가하고 한국어·영어 `InfoPlist.strings`를 번들 리소스로 포함한다. README의 Windows 전용 안내와 지원 운영체제 문구를 실제 지원 상태와 macOS 14 기준에 맞춘다.

## 캡처 백엔드

### 공용 호출 계약

현재의 독립 함수 호출을 캡처 단위 컨텍스트로 묶는다.

```text
CaptureContext::new(monitors)
  -> 권한 확인
  -> 플랫폼별 디스플레이/창 스냅샷 준비

context.capture_rect(rect)
context.list_windows()
```

Windows `CaptureContext`는 현재 GDI 구현을 그대로 호출하는 가벼운 래퍼다. macOS `CaptureContext`는 한 번 조회한 `SCShareableContent`와 디스플레이 매핑을 같은 캡처의 전체 화면, 모니터별 동결 프레임, 창 목록에서 재사용한다. `foreground_window`와 `restore_foreground`는 기존 계약을 유지한다.

### macOS 픽셀 획득

```text
start_capture
  -> 이전 활성 앱과 Tauri monitor geometry 저장
  -> screenshot-capture worker thread
  -> CaptureContext::new
     -> 화면 기록 권한 preflight/request
     -> SCShareableContent 비동기 조회를 제한 시간 안에 수신
     -> SCDisplay와 Tauri monitor 매핑
  -> 각 대상 rect
     -> 포함하는 display 선택
     -> display-local point sourceRect 계산
     -> 출력 width/height를 물리 픽셀로 지정
     -> cursor off, SDR 8-bit 설정
     -> SCScreenshotManager 비동기 CGImage 수신
     -> 알려진 RGBA bitmap context로 draw하고 alpha를 255로 정규화
  -> 기존 CapturedImage 또는 CaptureSession에 전달
```

Objective-C 객체는 캡처 worker thread 안에서만 소유한다. completion handler는 결과를 같은 worker가 기다리는 제한 시간 채널로 전달하며, 메인 UI thread를 막지 않는다. 권한 요청, shareable content 조회, 디스플레이별 픽셀 획득, 전체 동결과 overlay ready 시간을 각각 로그로 남긴다.

오버레이는 모든 픽셀 획득이 끝난 뒤 기존처럼 hidden 상태로 만들고 첫 paint 확인 후 표시한다. 따라서 선택 오버레이 자체는 원본 프레임에 포함되지 않는다. `showsCursor`는 Windows와 맞춰 끈다. DRM 또는 OS 보호 콘텐츠는 운영체제 정책을 따르며 별도의 우회는 하지 않는다.

macOS 오버레이 창과 페이지 배경은 첫 캔버스 프레임 전까지 투명하게 두어 검은 전체 화면 프레임을 피한다. 영역 선택을 확정한 뒤에는 오버레이를 계속 표시하면서 편집기를 준비한다. 편집기 창을 먼저 표시하고 오버레이 창을 숨긴 뒤 포커스를 편집기로 옮긴다. 오버레이의 세션 초기화 이벤트는 창을 숨긴 다음에 보내어 빈 캔버스가 화면에 노출되지 않게 한다. 이 인계 동안 새 캡처 시작은 막고, 편집기 열기 실패 시 오버레이와 캡처 상태를 함께 정리한다.

사용자가 정식 Mac 앱에서 이 전환의 깜빡임이 사라졌다고 확인했다. 남은 약 1초 체감 지연은 픽셀 획득 외 단계로 나누어 측정한다. 선택 오버레이는 WebView에서 원본 RGBA를 `ImageBitmap`으로 한 번 더 복사하지 않고 `ImageData`를 캔버스에 직접 그린다. 확대경은 그 캔버스를 소스로 사용하고, 최종 편집·출력 픽셀은 기존 Rust 원본을 유지한다. 화면별 React 코드를 지연 로딩하여 매번 생성하는 오버레이·편집기 창이 다른 화면의 코드를 함께 파싱하지 않게 한다. Rust 창 준비와 WebView의 프레임 수신·그리기·편집기 첫 그리기 시간을 로그로 남겨 실기 반복 측정 후 다음 병목을 결정한다.

## 권한과 오류 처리

`is_supported()`는 macOS 14 이상에서 `true`를 반환해 메뉴와 단축키를 노출한다. 권한 여부는 지원 여부와 분리한다.

사용자가 단축키, 트레이 또는 설정에서 캡처를 명시적으로 시작하면 다음 순서로 처리한다.

1. `CGPreflightScreenCaptureAccess`로 현재 권한을 확인한다.
2. 미허용이면 `CGRequestScreenCaptureAccess`를 한 번 호출한다.
3. 여전히 미허용이면 캡처 상태를 정상 종료하고 Preferences의 `permissions.screenRecording`을 연다.
4. 사용자에게 재시도 또는 앱 재시작이 필요한 상태를 한국어·영어로 안내한다.

Preferences와 Onboarding의 permission schema에 `screenRecording`을 추가한다. 기존 폴링 방식으로 시스템 설정에서 돌아왔을 때 상태를 갱신한다. 권한 거부, API 오류, 응답 제한 시간 초과는 서로 구분해 로그와 사용자 메시지에 남긴다.

## 디스플레이와 좌표

공용 계약은 계속 물리 픽셀의 왼쪽 위 원점을 사용한다. macOS의 display point 공간은 백엔드 경계에서만 다룬다.

각 `SCDisplay`에 대해 display ID, point frame, pixel width/height, 계산 scale을 수집한다. 이를 Tauri `MonitorGeometry`와 크기, scale, 주 디스플레이 기준 상대 배치로 매칭한다. 변환 함수는 다음을 보장한다.

- 대상 rect가 한 디스플레이 안에 있는지 검증한다.
- Tauri 전역 물리 픽셀 offset을 해당 display 로컬 point offset으로 변환한다.
- ScreenCaptureKit 출력 크기는 요청한 물리 픽셀과 정확히 일치한다.
- 음수 x/y, 세로 배치, 서로 다른 scale factor에서 반올림 오차를 한 픽셀 이내로 제한한다.
- 메뉴바와 Dock은 전체 화면 캡처에 보이는 상태 그대로 포함하며 work area는 편집기 배치에만 사용한다.

단위 테스트는 1x, 2x, 혼합 배율, 좌우·상하·음수 배치, 경계 교차, 마지막 영역 재사용을 포함한다. 실제 Mac의 2x 4K 화면에서 픽셀 크기를 확인하고, 내장 화면을 함께 활성화할 수 있을 때 혼합 배율 다중 모니터를 최종 검증한다.

## 창 선택과 포커스

Window Server의 onscreen window 목록을 front-to-back 순서로 읽는다. 다음 항목은 후보에서 제외한다.

- 현재 프로세스가 소유한 Ultra Clipboard 창
- 바탕 화면 또는 일반 창이 아닌 레이어
- 보이지 않거나 alpha가 0인 창
- 최소 크기 이하의 빈 사각형

창 bounds는 디스플레이별 좌표 변환을 거쳐 공용 물리 픽셀 `PixelRect`로 변환한다. 여러 화면에 걸친 창은 기존 overlay가 모니터별 교집합을 계산한다.

`NSWorkspace.frontmostApplication`의 PID를 저장하고 취소 시 `NSRunningApplication`을 다시 활성화한다. 편집기를 여는 성공 경로에서는 편집기가 포커스를 가져가는 기존 동작을 유지한다.

## OCR

`ocr.rs`에 `#[cfg(target_os = "macos")]` 구현을 추가한다.

```text
editor RGBA
  -> 입력 길이 검증
  -> 작은 이미지는 기존 정책에 맞춰 선택적으로 확대
  -> 메모리 PNG
  -> VNImageRequestHandler
  -> VNRecognizeTextRequest(accurate, automatic language detection, correction)
  -> observations의 최상위 후보 추출
  -> bounding box 기준 읽기 순서 정렬
  -> 줄바꿈 문자열 반환
  -> 기존 export OCR 경로가 클립보드에 복사
```

편집기의 `isWin` 제한을 제거해 macOS에서도 OCR 버튼과 `⌘⇧O`를 노출한다. 한국어와 영어가 함께 있는 샘플, 빈 이미지, 작은 UI 글자, 여러 줄 순서, 잘못된 픽셀 길이를 검증한다.

## 실제 Mac 작업 경로

구현 브랜치를 만든 뒤 Mac에 별도 checkout을 준비하고 그 브랜치를 동기화한다. SSH는 `/bin/zsh -lic`를 사용해 Homebrew Rust와 Node 경로를 로드한다. `npx pnpm@10.33.1`로 의존성을 설치하고 Rust/Frontend 검사와 Apple Silicon 빌드를 수행한다. Intel target도 같은 Mac 또는 CI에서 빌드한다.

실행 QA는 `/Applications/Ultra Clipboard.app` 하나를 현재 빌드로 갱신해 진행한다. 이전 별도 QA bundle은 사용자 요청에 따라 제거하고 해당 bundle ID의 권한 기록도 초기화했다. 정식 앱의 Application Support 데이터는 보존한다. ad hoc 서명으로 새 빌드를 설치하면 macOS가 기존 권한을 현재 실행 코드에 적용하지 않을 수 있으므로, 화면 기록·손쉬운 사용·전체 디스크 접근의 정식 bundle ID 권한만 초기화한 뒤 사용자가 시스템 설정에서 다시 허용하고 앱을 재시작한다. 보안 프롬프트와 권한 스위치는 사용자가 직접 조작한다.

## 성능 검증

권한 승인과 첫 framework 초기화 이후를 warm 상태로 정의한다. 실제 4K Retina Mac에서 다음을 수집한다.

- 영역 캡처 10회: permission, shareable content, pixel capture, frame freeze, overlay ready 단계
- 전체 화면 10회: pixel capture부터 editor ready까지
- 창, 반복 캡처 각 5회
- 의도된 3초 대기를 제외한 지연 캡처 5회

각 모드의 중앙값과 p95, 성공률, 출력 해상도를 기록한다. 단일 4K 디스플레이의 warm 영역 캡처는 backend freeze p95 750ms 이하, overlay ready p95 1초 이하를 목표로 한다. 목표를 넘으면 단계 로그로 병목을 확인해 같은 조건에서 다시 측정한다. 검은 프레임, stale frame, 오버레이 포함, UI 멈춤은 한 번이라도 발생하면 미완료다.

## 호환성 및 회귀

- 설정, 데이터베이스 schema, 저장 데이터 형식, 이벤트명과 command 이름은 변경하지 않는다.
- Windows GDI 캡처, 창 필터, OCR 동작은 같은 `CaptureContext` 계약 아래 유지한다.
- 캡처 메뉴와 단축키는 기존 설정 키를 그대로 사용한다.
- README와 다음 릴리스 changelog에서 macOS 지원 상태와 최소 버전을 일치시킨다.
- macOS 앱 package의 code signing과 notarization은 현재 릴리스 정책을 유지한다. 실제 배포 전에 안정적인 서명 identity가 있는지 확인한다.

## 위험과 대응

- **혼합 배율 좌표 오차:** 변환을 독립 함수로 분리하고 픽셀 크기와 경계 반례를 테스트한 뒤 실제 두 화면에서 확인한다.
- **Objective-C 객체의 thread 제약:** 객체를 worker thread 안에 한정하고 채널에는 소유 가능한 픽셀 또는 오류만 넘긴다.
- **권한 승인 후 즉시 반영되지 않음:** 설정 폴링, 재시도 안내, 필요 시 재시작 안내를 제공한다.
- **ad hoc 재서명 후 권한 불일치:** 로컬 설치마다 코드 identity가 바뀔 수 있다. 현재 Mac에는 코드 서명 인증서가 없으므로 앱별 권한 초기화와 사용자 재허용으로 QA하고, 안정적인 배포 서명 없이 권한 유지가 검증됐다고 주장하지 않는다.
- **비동기 캡처 hang:** shareable content와 screenshot completion에 제한 시간을 둬 캡처 상태를 반드시 해제한다.
- **Intel 전용 컴파일 오류:** x86_64 target 빌드와 macOS CI를 필수 gate로 둔다.
- **현재 실제 화면이 한 대뿐임:** 혼합 배율은 자동 테스트로 먼저 보장하고 MacBook 내장 화면이 활성화된 시점에 실제 QA gate를 완료한다.

## 롤백

DB migration이나 사용자 데이터 변경은 없다. 작업 브랜치에서 macOS backend, OCR 분기, permission UI, bundle 설정과 새 macOS 전용 의존성을 되돌리면 기존 v1.2.2 상태로 복귀한다. 정식 앱의 클립보드 기록·설정은 갱신 중에도 Application Support 경로에 그대로 보존한다.
