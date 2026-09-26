# ADR-0003: 웹뷰는 앱의 타입 명령과 이벤트 듣기만 할 수 있다

상태: 채택, 2026-09-26.

## 배경

Tauri 2는 웹뷰가 부를 수 있는 명령을 capability 파일의 권한 목록으로 정한다. 사용자의 UAC
원격 승인기는 `core:default`를 넣지 않고 앱 자신의 명령만 나열하며, 등록된 명령과 권한 목록이
어긋나면 실패하는 테스트를 둔다. CSP는 `default-src 'self'`에 IPC 출처만 연다.

## 결정

1. `host/app/capabilities/main.json`은 `windows: ["main"]`에 앱 명령의 `allow-*` 권한과
   `core:event:allow-listen`, `core:event:allow-unlisten`만 갖는다. `core:default`, 플러그인
   권한(`dialog:*`, `fs:*`, `shell:*`, `http:*`, `opener:*`)은 없다. `remote`는 없다.
2. `app.withGlobalTauri`는 `false`다. 웹뷰 코드는 `@tauri-apps/api/core`의 `invoke`와
   `@tauri-apps/api/event`의 `listen`만 `bridge.ts`에서 쓴다.
3. CSP는 UAC와 같다.
   `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self';
   connect-src ipc: http://ipc.localhost https://ipc.localhost; object-src 'none'; base-uri 'self';
   frame-ancestors 'none'; form-action 'none'`. 개발 CSP는 여기에 Vite 개발 서버
   (`ws://127.0.0.1:1420 http://127.0.0.1:1420`)와 `style-src 'unsafe-inline'`만 더한다.
4. 창은 `create: false`로 선언하고 `setup`에서 `WebviewWindowBuilder::from_config`로 만들며,
   `on_navigation`은 `tauri://localhost`, `http(s)://tauri.localhost`, 그리고 디버그 빌드의
   `http://127.0.0.1:1420`만 허용한다.
5. 웹뷰가 원시 문자열로 경로, URL, 명령줄을 보내는 명령은 없다. 파일 선택은 Rust가 대화상자를
   열어 경로를 얻는다. 다운로드 URL은 매니페스트에서만 나온다.
6. `host/tools/tauri-capability.test.mjs`가 (a) `generate_handler!`의 모든 명령이 권한 목록에
   있고, (b) 권한 목록의 모든 `allow-*`가 등록된 명령이며, (c) `core:` 권한이 listen과 unlisten
   두 개뿐이고, (d) 자동 생성 권한 파일이 생성기 원문 그대로인지 검사한다.
7. 게스트 화면 위에 그리는 것은 없다. 게스트 창은 네이티브 자식 창이라 웹뷰가 그 위에 그릴 수
   없고(airspace), 키 안내 오버레이가 필요하면 별도의 투명 레이어드 창을 Rust가 만든다.

## 결과

- 웹뷰가 침해되어도 할 수 있는 일은 앱 명령 호출뿐이고, 그 명령들은 인자를 검증한다.
- 새 기능은 명령 하나를 Rust에 추가하고 권한 파일을 생성하는 절차를 거친다. 테스트가 그
  절차의 누락을 잡는다.
- 드래그 앤 드롭으로 APK를 넣는 기능은 Tauri의 드래그 이벤트가 주는 경로를 Rust가 받아
  검증하는 방식으로 한다. 웹뷰의 HTML5 drop은 파일 내용만 받으므로 쓰지 않는다.
