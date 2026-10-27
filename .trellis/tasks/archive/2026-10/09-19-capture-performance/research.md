# 조사·검증 기록

## 캡처

- Windows 격리 QA 앱에서 첫 `Area` 캡처의 화면 동결 Rust 로그는 1개 모니터, 532ms였다. 이 값은 GDI 화면 복사까지이며 오버레이 표시 완료 시간은 아니다.
- 수정 후 동일 QA 경로의 첫 캡처는 1개 모니터, 557ms였고 오버레이가 `Created -> Ready -> Visible`로 전환됐다. 화면 복사 시간은 25ms 길었으나 서로 다른 단일 표본이므로 성능 악화 또는 개선으로 해석하지 않는다.
- `screenshot::overlay::present`는 첫 캡처 때 숨긴 WebView를 만들고 세션 이벤트를 보냈다. 새 WebView의 `useMount`도 같은 세션을 요청하므로 타이밍에 따라 첫 프레임 IPC 요청이 두 번 발생할 수 있었다. 새 창은 mount에서만 읽고 재사용 창에만 이벤트를 보낸다.
- 이전 1.5초 fallback은 프론트의 첫 페인트 알림 없이 검은 배경 창을 보여줄 수 있었다. 이제 10초 동안 페인트하지 못하면 해당 세션을 취소·복구한다.
- 이전 `reveal`은 `show` 뒤 monitor bounds를 다시 적용하고 topmost를 재설정했다. 이제 두 창 상태를 표시 전에 적용한다. 혼합 DPI·다중 모니터는 실기기 재검증이 필요하다.
- 프레임은 이미 Tauri binary `Response`의 ArrayBuffer로 전달되고, `Uint8ClampedArray(buffer)`는 ArrayBuffer 뷰다. base64 전환이나 JS 배열 복사 문제는 확인되지 않았다.
- 데스크톱 자동화가 스크린샷을 제공하지 않고 QA 창 포커스를 얻지 못했다. 수정 후 깜빡임의 시각적 재현과 오버레이 표시 완료 시간의 전후 수치는 미측정이다. 두 번째 캡처 시도는 `screen copy failed: 핸들이 잘못되었습니다. (0x80070006)`로 실패해 화면 세션 문제가 있음을 확인했다.
- D: 저장소의 별도 QA 식별자 `com.toughcsb.ultraclipboard.qa.codex`로 앱을 다시 실행해 온보딩과 클립보드 창 진입을 확인했다. Orca `computer-use`의 스크린샷은 `screenshot_failed`였고, 창 포커스 요청도 `window_not_focused`여서 캡처 단축키를 실행하지 못했다. 따라서 실제 깜빡임과 다중 모니터 동작은 여전히 미검증이다.

## 앱 전체 경로

| 경로 | 확인한 근거 | 결과 |
| --- | --- | --- |
| 시작~창 표시 | `lib.rs` setup은 설정·DB·클립보드 초기화를 동기 완료한 뒤 단축키와 트레이를 설치한다. | 분리 계측이 없어 병목 판정 보류. |
| 단축키 | `shortcut::handle_event`는 직접 `window::toggle_window` 또는 `screenshot::start_capture`로 전달한다. | 별도 대기 타이머 없음. GUI 입력 지연은 미측정. |
| 목록 | `react-virtuoso`, 30행 페이지, 180행 캐시, 표시 범위 기준 사전 로드를 사용한다. | 전체 목록 렌더링은 없음. |
| DB 정렬 | QA DB의 `EXPLAIN QUERY PLAN`에서 기본 정렬은 `SCAN clipboard_items`, `USE TEMP B-TREE FOR ORDER BY`; QA 기록은 2행이었다. | 대량 데이터 성능 영향은 미측정. 인덱스 추가는 released schema 마이그레이션이므로 이번 변경에서 제외. |
| 검색 | 3자 이상 토큰은 FTS5, 짧은 검색은 escaped LIKE를 사용한다. | 짧은 검색은 데이터가 클 때 스캔할 수 있으나 미측정. |
| 이미지 | 누락 썸네일은 백그라운드 생성하며, 첫 목록 응답에는 원본 이미지 경로가 사용된다. | 첫 표시 시 큰 원본 디코딩 가능성은 있으나 실제 병목 미측정. |
| IPC·폴링 | 캡처 첫 창의 중복 IPC 가능 경로를 제거했다. Rust 클립보드 감시 주기는 120ms다. | 폴링 변경 근거는 없었다. |

## 검증

- `cargo fmt --check`: 통과.
- `cargo clippy -- -D warnings`: 통과.
- `cargo test`: 229 passed, 0 failed, 8 ignored. 8개는 시스템 클립보드·음향을 사용하는 테스트 등으로 기본 실행에서 제외됐다.
- `corepack pnpm build`: 통과. Vite는 2422 modules를 변환하고 500 kB 초과 chunk 경고를 출력했다.
