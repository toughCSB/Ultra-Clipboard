# 삭제 버튼 가시성과 클릭 영역

## Goal

클립보드 목록 항목의 삭제 빠른 동작을 더 쉽게 보고 클릭할 수 있게 한다.

## Background

- ClipboardQuickActions.tsx의 버튼은 size-5(20px), 아이콘은 text-sm이며 danger 토큰이 글자색에만 적용된다.
- 각 동작의 motion.span 너비는 1.25rem으로 고정돼 있다.
- DESIGN.md는 양 테마와 Ant Design 기반 색 토큰을 사용한다.

## Requirements

- 삭제 버튼에 Ant Design danger 계열 색상과 분명한 배경·대비를 적용한다.
- 버튼과 실제 클릭 영역을 키우되 카드 메타데이터 줄과 인접 동작 레이아웃을 보존한다.
- 라이트·다크 테마와 키보드 접근 경로를 확인한다.

## Acceptance Criteria

- [x] 삭제 버튼의 클릭 영역이 기존 20×20px보다 커지고 색상으로 구분된다.
- [x] 다른 동작과 함께 있을 때 넘침이나 겹침이 없다.
- [ ] 양 테마의 기본·hover 대비를 확인했고 focus ring의 실제 키보드 표시는 남았다.
