# M3 증거: 설치기, 릴리스 파이프라인, 설치된 제품의 완주

작성 2026-10-01. 완료 기준은 CLAUDE.md M3(사용자 승인 2026-10-01)이고 설계는 ADR-0011이다. 아래 절은 기준 순서다.

## 1. 로컬 설치기 빌드와 설치 (개발 PC, 2026-10-01 13:15~13:30)

러너 QEMU 릴리스가 나오기 전에 설치기의 기계적인 부분을 개발 PC에서 먼저 확인했다. `host/qemu-dist`에 개발 트리의
QEMU 묶음(`host/target/debug/qemu`, ome3와 같은 패치 집합의 진단 빌드, 158개 파일 436 MB)을 복사하고
`cargo build -p ome-setup --release` 뒤 `host/app`에서 다음을 돌렸다.

```
node ../node_modules/@tauri-apps/cli/tauri.js build --ci --config ../../ci/release/tauri.release.conf.json
```

| 시도 | 설정 | 결과 |
|---|---|---|
| 1 (13:15) | `installMode: perMachine`(바꾸기 전 설정) | 설치기 51.8 MB. `/S`로 띄우자 즉시 "사용자가 작업을 취소했습니다"(승격 요구, 세션에서 UAC 승인 불가). 범위 결정대로 `currentUser`로 바꿈 |
| 2 (13:25) | `installMode: currentUser` | 설치기 51.8 MB. `/S` 설치 종료 코드 0, UAC 없음 |

설치된 배치(`local-install-layout.txt`): `%LOCALAPPDATA%\Open Mobile Emulator`에 `ome.exe`(17.9 MB), `ome-setup.exe`,
`uninstall.exe`, `manifests\artifacts.json`, `manifests\images\bliss-16.9.7-android-13.json`, `qemu\`(158개 파일,
`bin\qemu-system-x86_64.exe`와 `share\qemu\edk2-x86_64-code.fd` 확인). 현재 사용자의 제거 항목(`HKCU\...\Uninstall`)에
`DisplayVersion 0.1.0`, `InstallLocation`이 등록되고, 시작 메뉴와 바탕 화면에 바로 가기가 생겼다. 설치기 크기 51.8 MB는
436 MB 묶음을 NSIS가 LZMA로 눌러 담은 결과다. 자원 덧판의 폴더 매핑(`"../qemu-dist": "qemu/"`)이 구조를 보존한다는
것(ADR-0011 3번)을 이 배치로 확인했다.

## 2. 설치된 제품의 완주 (완료 기준 2)

### 34회차: 로컬 설치기(위 2번 시도)로 설치한 제품, 개발 PC, 새 홈 (13:26, 실패)

`run-dod-installed.ps1 -Round 34 -SeedFrom dod-home-33`(ISO만 복사). 첫 화면, 호스트 점검, WHPX, 이미지 검증을 지나
`설치하기` 뒤 S1.5에서 실패했다. 제품은 "설치 도우미가 끝나기 전에 멈췄습니다"를 보였지만 도우미의 시리얼 로그
(`round34-install-serial-tail.txt`)는 `OME-INSTALL done`으로 끝났고 디스크에는 설치가 되어 있었다. 드라이버가 본 마지막
진행은 `copy 82`였고 실패 보고의 백분율이 100이었으므로 런타임은 `done`을 읽었다. 실패 판정의 원인은 감독자다.
`ome-supervisor`의 생애 주기는 `Running`에서 프로세스가 끝나면 종료 코드와 무관하게 `Failed`로 가고, 정상 종료는 QMP의
`SHUTDOWN` 이벤트를 먼저 받아 `Stopping`이 되어 있을 때만 `Stopped`가 된다. 도우미의 `poweroff -f` 뒤 QEMU가 이벤트를
보내고 바로 소켓을 닫으면 이벤트가 유실될 수 있어(33회차는 받았고 34회차는 못 받았다) 깨끗한 종료가 실패로 분류됐다.
고친 것: (1) 생애 주기에서 `Running` + 종료 코드 0은 `Stopped`다(크래시는 0이 아니다). (2) 런타임의 설치 마무리는 시리얼
로그의 `done`을 성공의 근거로 삼고 감독자 상태를 요구하지 않는다. 같은 유실은 일반 게스트가 스스로 꺼질 때 "예기치 않게
끝났습니다" 알림을 잘못 내는 원인이기도 하다.

## 3. 러너 QEMU 빌드와 출처 증명 (ADR-0007, R4)

`qemu-release.yml`을 접미사 `ome4`로 수동 실행했다(https://github.com/115dkk/Open-Mobile-Emulator/actions/runs/36813804342,
커밋 73aa3a5, 러너 `windows-2025`, 2026-10-01 04:08~04:33 UTC, 25분). 첫 실행(36813578129)은 릴리스 이름 확인 단계가
`gh release view`의 실패 코드를 되풀이해 1로 끝난 것이라 `exit 0`을 더해 다시 돌렸다.

| 단계 | 결과 |
|---|---|
| MSYS2 UCRT64 준비와 `build-qemu.sh all` | 04:10 시작. 의존 패키지 설치, v11.1.1 복제와 펌웨어 서브모듈, 패치 0001~0005 적용, ninja 2339 대상(04:18:10 진입, 04:28:39 `qemu-system-x86_64.exe` 링크), 런타임 복사와 DLL 해석, 소스 묶음 84,035개 항목(04:30:55) |
| 산출물 | `qemu-ome-v11.1.1-c3d48b7d1e89-win64.zip` 62,776,149 B(SHA-256 `ba2a53f3…15db42`), `qemu-source-offer-v11.1.1-c3d48b7d1e89.tar.gz` 249,537,255 B(`8b737995…3ac380`), `THIRD_PARTY.generated.md`, `ome-patches.txt`, `pacman-lock.txt`, `SHA256SUMS` |
| 출처 증명 | `actions/attest@v4`가 여섯 파일 모두에 붙임 |
| 릴리스 | 사전 릴리스 `qemu-v11.1.1-ome4`, 태그는 워크플로 커밋 73aa3a5 |

러너의 MSYS2 패키지는 개발 PC보다 새것이라 번들 DLL 가운데 libpng(1.6.58→1.6.59), libwinpthread(r420→r426), pcre2(10.48→10.49)가
바뀌었고, 그 표(`THIRD_PARTY.generated.md`)를 저장소의 `THIRD_PARTY.md` 생성 블록에 옮겨 적었다. `manifests/qemu-release.json`이 이
릴리스의 두 파일과 SHA-256을 고정하며, `m2-dod.yml`도 ome3 대신 이 zip을 받는다. 개발 PC에서 만든 ome1~ome3는 더 쓰지 않는다.

## 4. 제품 릴리스 v0.1.0 (완료 기준 1, 4)

(태그 뒤에 적는다.)

## 5. 러너에서의 설치기 완주 (완료 기준 2, `m3-release-dod.yml`)

(릴리스 뒤에 적는다.)
