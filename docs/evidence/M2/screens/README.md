# M2 화면 스크린샷 (2026-09-27)

QA 갤러리(`host`에서 `npm run build:qa` 뒤 `vite preview --mode qa`, `target/ui-qa/qa.html?state=<id>`)를
헤드리스 Chrome으로 1280x800에 찍은 것이다. 시스템 테마(밝은 테마)로 찍혔고, 오른쪽 아래의
상태/테마 선택기는 갤러리의 것이지 제품 화면이 아니다.

주의: 이 여덟 장은 화면 워커 D(계약 3판 버튼 연결, 커밋 09706ef) 이전에 빌드한 번들로 찍었다.
그래서 마법사의 `나중에 하기`가 없고, 마법사 바닥 버튼(`다시 확인`, `계속`, `활성화`)은 DOM에는
있으나 갤러리 선택기가 차지하는 높이 때문에 800px 뷰포트 아래로 밀려 보이지 않는다. 제품
창(선택기 없음)에서는 바닥에 붙는다. 어두운 테마와 D 이후 상태는 갤러리를 직접 열어 본다.

| 파일 | 상태 |
|---|---|
| host-ready.png | S1.1 호스트 점검(사용 가능) |
| whpx-consent.png | S1.2 하이퍼바이저 활성화 |
| download-transferring.png | S1.4 다운로드 중 |
| stage-running.png | S2 실행 중(무대는 검정, 실제 창은 창 담기 뒤에 보인다) |
| stage-failed-boot.png | S2 부팅 실패 |
| input-running.png | S4 입력 |
| display-refresh.png | S5 표시(주사율 절 포함) |
| settings-running.png | S6 설정 |
