# M2 화면 스크린샷 (2026-09-27)

QA 갤러리(`host`에서 `npm run build:qa` 뒤 `vite preview --mode qa`, `target/ui-qa/qa.html?state=<id>`)를
헤드리스 Chrome으로 찍은 것이다. 번들은 화면 워커 D(계약 3판 버튼 연결, 커밋 09706ef) 이후의 것이고,
갤러리에 `theme=dark|light`와 `picker=hidden` URL 인자를 더한 커밋과 같은 소스다. 그래서 상태/테마
선택기는 화면에 없고, 마법사 바닥 버튼(`나중에 하기`, `다시 확인`, `계속`, `활성화`, `취소`, `다음`,
`건너뛰기`, `완료`)이 모두 보인다.

크기는 제품 창 기본값인 1280x800이다. 레일 화면 넷은 내용이 그보다 길어 `-tall` 판을 따로 두었다
(입력과 표시는 1280x1600, 설정은 1280x2400). 무대는 검정으로 보이는데, 실제 게스트 창은 창 담기
스파이크 뒤에 그 자리에 들어온다.

찍은 명령(PowerShell이나 bash 어느 쪽이든 경로는 슬래시로 쓴다. 역슬래시 뒤의 `$id`는 bash 큰따옴표
안에서 풀리지 않아 이전 세션에서 `screens$s.png` 같은 파일이 생겼다):

```
chrome.exe --headless=new --disable-gpu --hide-scrollbars --no-first-run
  --user-data-dir=<temp>/chrome-profile --window-size=1280,800 --virtual-time-budget=4000
  --screenshot=<temp>/shots/<id>-<theme>.png
  "http://127.0.0.1:4173/qa.html?state=<id>&theme=<theme>&picker=hidden"
```

| 파일(`-dark`, `-light` 두 장씩) | 상태 |
|---|---|
| host-ready | S1.1 호스트 점검(사용 가능) |
| whpx-consent | S1.2 하이퍼바이저 활성화 |
| download-transferring | S1.4 이미지 다운로드 중 |
| install-guide | S1.5 설치 안내 |
| first-boot-done | S1.6 첫 부팅, 기능 확인 끝 |
| app-install | S1.7 앱 설치(설치 중 한 건, 완료 한 건) |
| blocked-virtualization | S8 가상화 꺼짐 |
| stage-running | S2 실행 중 |
| stage-running-auto | S2 실행 중, 자동 적용과 매핑 일시 중지, fps |
| stage-failed-boot | S2 부팅 실패 |
| apps-running | S3 앱 목록 |
| input-running | S4 입력(`-dark-tall` 추가) |
| display-refresh | S5 표시, 주사율 절 포함(`-dark-tall` 추가) |
| settings-running | S6 설정, 실행 중(`-dark-tall` 추가) |
| settings-update-failed | S6 설정, 꺼짐, 업데이트 확인 실패(`-dark-tall` 추가) |

## 살펴본 결과

1280x800에서 잘리거나 겹치는 요소는 없다. 어두운 테마의 보조 문구(`--muted`)와 비활성 버튼은
읽을 수 있고, 밝은 테마도 같은 배치다. 앞선 여덟 장의 README는 마법사 바닥 버튼이 갤러리 선택기의
높이 때문에 800px 아래로 밀렸다고 적었는데, 선택기는 `position: fixed`라 높이를 차지하지 않는다.
바닥 버튼은 제자리(720~800px)에 있었고 선택기가 그 위를 덮었을 뿐이다. 제품 창에는 선택기가 없다.

상태별로 확인한 것:

- 마법사 일곱 단계 모두 왼쪽 아래에 `나중에 하기`가 있고, 다운로드 중과 설치 안내의 주 버튼은
  비활성으로 그려진다.
- 무대 실행 중은 도구 막대(스크린샷, 볼륨, 해상도, 매핑 표지, 편집, 다시 시작, 끄기)와 상태 표시줄이
  있고, 자동 적용 상태는 `매핑 일시 중지 (F12)`와 fps를 상태 표시줄 오른쪽에 보인다.
- 설정은 운영체제, 설치된 운영체제, 입력, 고급(adb 주소와 도움말, 다른 PC 연결, 루트 권한은 이
  이미지에서 비활성, fps), 저장 위치, 업데이트, 진단(최근 이벤트), 정보 순이다.
- 입력 화면의 매핑 표는 1280x800에서 여섯 줄까지 보이고 나머지는 스크롤이다. 1600 판에서는 여덟 줄이
  다 보인다.
