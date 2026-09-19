# Design

- Rust PlatformAutostart가 OS 등록을 소유한다. React 토글·설정 키·로케일은 기존 계약을 재사용한다.
- Windows AutoLaunchBuilder의 WindowsEnableMode::CurrentUser를 유지하고 HKCU Run·StartupApproved만 정리한다. HKLM 탐색·정리 분기를 제거한다.
- 기존 general.autoStart와 serde 기본값을 유지해 마이그레이션이나 새 계약을 만들지 않는다.
- macOS auto-launch 백엔드는 기존 LaunchAgent 동작을 유지하고 cfg·crate 소스를 검토한다.
- 과거 HKLM 값은 앱이 변경하지 않는다. 남아 있을 경우 시작 동작에 영향을 줄 수 있음을 보고한다.
