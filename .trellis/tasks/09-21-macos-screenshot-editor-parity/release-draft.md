# Release draft: v1.2.3 candidate

This is a draft for the next release. Do not tag or publish until the task's release gates pass. When the version is chosen, move the English section to `CHANGELOG.md`, synchronize `CHANGELOG.ko-KR.md`, and update the version files together.

## English

### Added

- Enable area, window, full-screen, delayed, and repeat capture on macOS 14 and later, with the existing screenshot editor and Vision OCR.
- Show screen recording permission in macOS onboarding and preferences.
- Add highlighter intensity controls for new and selected annotations on Windows and macOS.

### Changed

- Keep the frozen capture visible until the editor paints to avoid desktop flicker, and load screenshot pages separately to shorten window startup.
- Replace the editor discard action with a larger red trash icon and make the primary copy action more prominent.

### Release notes

- macOS display coordinates on mixed-scale hardware and repeated end-to-end capture latency still require device verification.
- A stable macOS distribution signature and notarization are required before publishing this update so system permission grants remain valid across app updates.

## 한국어

### 추가

- macOS 14 이상에서 영역·창·전체 화면·지연·반복 캡처를 제공하고 기존 캡처 편집기와 Vision OCR을 연결했습니다.
- macOS 온보딩과 설정에 화면 기록 권한 상태를 추가했습니다.
- Windows와 macOS에서 새 형광펜 표시와 선택한 표시의 진하기를 조절할 수 있게 했습니다.

### 변경

- 편집기 첫 화면이 그려질 때까지 캡처 화면을 유지해 바탕 화면 깜빡임을 줄이고, 캡처 화면을 분리해 창 시작 시간을 줄였습니다.
- 편집기의 버리기 동작을 더 큰 빨간 휴지통 아이콘으로 바꾸고 주 복사 버튼을 강조했습니다.

### 릴리즈 안내

- 배율이 서로 다른 실제 디스플레이의 좌표와 캡처 전체 흐름의 반복 지연 측정은 기기 검증이 남아 있습니다.
- 앱 업데이트 후 시스템 권한이 유지되도록 안정적인 macOS 배포 서명과 공증을 갖춘 뒤 이 업데이트를 배포해야 합니다.
