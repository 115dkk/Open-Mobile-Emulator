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

### 35회차: 로컬 설치기(ome4 런타임 동봉, 위 두 수정 반영)로 설치한 제품, 개발 PC, 새 홈 (13:46~14:04, 통과)

설치기 54.3 MB(`host/qemu-dist`에 `qemu-v11.1.1-ome4`의 런타임을 풀어 동봉. 설치된 `qemu-system-x86_64.exe`의
SHA-256이 릴리스 zip 안 `SHA256SUMS`의 값 `62df3d14…6f890`과 같음). `run-dod-installed.ps1 -Round 35 -SeedFrom dod-home-33`.
드라이버의 결과는 `needs-review`(전 단계 통과, 게임 화면은 사람이 본다)이고 실패 단계는 없다(`round35-dod-result.json`).

| 단계 | 시간 | 비고 |
|---|---|---|
| S1.1 호스트 점검, S1.2 WHPX, S1.4 이미지 | 1.6초, 1.5초, 3.1초 | ISO는 복사해 둔 것이라 검증만 |
| S1.5 설치 | 31.1초 | `설치하기` 뒤 입력 없음. 설치된 제품의 `qemu\bin`과 `share\qemu` 펌웨어로 도우미 부팅(`round35-qemu-install-cmd.txt`). 시리얼 로그 `done`(`round35-install-serial-tail.txt`), 기록 `install.state: installed`, `boot: direct`(`round35-guest.json`) |
| S1.6 첫 부팅 | 47.4초 | 직접 커널 부팅(`round35-qemu-boot-cmd.txt`) |
| S1.7 앱 설치 | 20.8초 | |
| 게임 실행 | 15.6분 | 1분에 권한 대화상자(Allow 탭), 3분부터 게임 화면. 15분 동안 게임 화면이 유지됨 |

마지막 화면(`13-game-final-stage`)은 제품 창 안의 게임 제목 화면(Ver 1.6.44)에 게임의 "서버와 통신이 되지 않습니다.
네트워크를 확인해주세요" 대화상자가 떠 있는 상태였다. 33회차(같은 PC, 개발 빌드)는 데이터 다운로드 안내까지 갔으므로,
ome4 런타임의 게스트 네트워크가 원인인지 그 시각의 호스트 회선이나 게임 서버 사정인지 아래에서 따로 가렸다.

**게스트 네트워크 판정(`guest-network-by-binary-path.txt`)**: 35회차의 디스크를 같은 명령으로 화면 없이 부팅해 adb로 시험했다.
설치된 제품의 QEMU(ome4)에서는 게스트가 호스트(10.0.2.2)와 8.8.8.8에 ICMP는 되지만 슬러프의 DNS(10.0.2.3)를 거친 이름
풀이와 바깥 TCP 443이 모두 실패했고, 개발 묶음(`host\target\debug\qemu\bin`)에서는 같은 디스크로 이름 풀이와 TCP 443이
됐다. 그런데 ome4의 exe와 개발 DLL, 개발 exe와 ome4 DLL을 섞은 두 조합도 실패했고, **개발 묶음을 그대로 새 경로에 복사한
것도 실패했다.** 바이너리가 아니라 실행 파일의 경로가 결과를 바꾼다. 이 PC는 Windows 방화벽이 꺼져 있고 AhnLab V3 365
Clinic이 방화벽 제품으로 켜져 있어, 그 프로그램별 바깥 연결 제어가 전에 허용된 경로(개발 묶음)만 통과시키고 새 경로의
QEMU는 막는 것으로 판단한다. 게임의 서버 통신 오류는 그 때문이고 ome4 런타임의 결함이 아니다. 제품 쪽에서 할 일은
없고, 이 PC에서 설치된 제품으로 게임까지 보려면 사용자가 AhnLab에서 설치 폴더의 `qemu-system-x86_64.exe`를 허용해야
한다. 러너에는 그런 제품이 없으므로 릴리스 설치기의 완주(5절)가 깨끗한 증거가 된다. `docs/KNOWN_LIMITATIONS.md`와
`docs/help/install.md`에 증상과 대처를 적었다.

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

태그 `v0.1.0`은 처음 67efcb8에 밀었다가 `release.yml`의 verify 잡이 ci.yml과 같은 이유로 실패해(`ome-guest-install`의 테스트가
저장소가 무시하는 `.log` 픽스처를 `include_str!`로 읽어 러너에서 컴파일되지 않음. 2026-09-30부터 CI가 빨갰던 두 원인 가운데
하나이고, 다른 하나는 철회된 `yoke-derive`였다) 픽스처를 `.txt`로 바꾼 e79c270으로 옮겨 다시 밀었다. 그 실행이
https://github.com/115dkk/Open-Mobile-Emulator/actions/runs/36819100622 이다(2026-10-01 05:18~05:37 UTC).

| 잡 | 시간 | 내용 |
|---|---|---|
| verify | 8분 9초 | `ci/Invoke-AllChecks.ps1`(정책 검사, Pester 48개)과 `node tools/quality.mjs`(UI 테스트 249개, fmt, clippy, cargo test) |
| qemu-source-offer | 48초 | `Get-QemuBundle.ps1`: ome4의 zip과 소스 묶음을 받아 SHA-256 대조, `gh attestation verify --signer-workflow qemu-release.yml` 통과, 산출물로 넘김 |
| product | 11분 3초 | ome-setup 빌드, `tauri build --config ci/release/tauri.release.conf.json`, `Stage-ReleaseAssets.ps1`, `Check-ReleaseAssets.ps1`(findings=0), `actions/attest@v4`, `gh release create --prerelease`, 올린 자산을 다시 받아 `gh attestation verify`(`release-v0.1.0-verify-runner.txt`, 실패 0) |

릴리스 https://github.com/115dkk/Open-Mobile-Emulator/releases/tag/v0.1.0 (사전 릴리스)의 자산은 다음과 같고 `SHA256SUMS`에 전부 적혀 있다.

| 자산 | 크기 | SHA-256 |
|---|---|---|
| `Open-Mobile-Emulator-0.1.0-x64-setup.exe` | 54,269,596 B | `df8624de…f43117` |
| `Open-Mobile-Emulator-0.1.0-x64-setup.exe.sha256` | 107 B | |
| `qemu-ome-v11.1.1-c3d48b7d1e89-win64.zip` | 62,776,149 B | `ba2a53f3…15db42` (3절과 같은 파일) |
| `qemu-source-offer-v11.1.1-c3d48b7d1e89.tar.gz` | 249,537,255 B | `8b737995…3ac380` (R4 소스 묶음) |
| `THIRD_PARTY.md`, `NOTICE`, `LICENSE`, `SHA256SUMS` | | M3 3번의 CI 검사(`Check-ReleaseAssets.ps1`)가 올리기 전에 확인 |

개발 PC에서 사용자 절차대로 다시 확인했다(`release-v0.1.0-verify-local.txt`). 내려받은 설치기의 SHA-256이 `.sha256`과 같고,
`gh attestation verify ... --signer-workflow .../release.yml`이 여덟 자산을 주제로 하는 증명을 돌려주며, 그 증명은 저장소
`115dkk/Open-Mobile-Emulator`의 `refs/tags/v0.1.0`(e79c270)과 워크플로 `release.yml`, 실행 36819100622를 가리킨다. 개발 PC에서
만든 파일은 릴리스에 없다(ADR-0007).

## 5. 러너에서의 설치기 완주 (완료 기준 2, `m3-release-dod.yml`)

`gh workflow run m3-release-dod.yml -f tag=v0.1.0`. 워크플로는 릴리스의 설치기와 `.sha256`을 받아 해시를 대조하고
`gh attestation verify --signer-workflow .../release.yml`을 통과시킨 뒤, m2-dod.yml과 같은 방식으로 표준 사용자 `omeuser`를
만들어 그 계정으로 설치기를 `/S`로 돌리고(`run-as-user.ps1 -Installer`) 설치된 `%LOCALAPPDATA%\Open Mobile Emulator\ome.exe`에
완료 실행 드라이버를 댄다.

| 시도 | 실행 | 결과 |
|---|---|---|
| 1 | https://github.com/115dkk/Open-Mobile-Emulator/actions/runs/36820731098 (05:38~05:42 UTC) | 설치기 검증 통과, 표준 사용자 설치 종료 코드 0, 설치된 제품으로 첫 화면·호스트 점검·WHPX 통과. S1.4에서 SourceForge 다운로드가 두 번 모두 한 바이트도 받지 못한 채 실패(`artifact_download_failed`, 드라이버의 두 번째 시도까지 47초). 제품 밖의 일이며 m2-dod.yml 23회에서도 같은 미러 실패가 있었다 |
| 2 | https://github.com/115dkk/Open-Mobile-Emulator/actions/runs/36821141980 (05:43~05:47 UTC) | 같은 자리에서 같은 실패. 그 시각 개발 PC에서도 `downloads.sourceforge.net`이 HTTP 522(Cloudflare, 원본 서버 응답 없음)를 돌려줬다(`sourceforge-522.txt`). 배포처 장애이며 제품이나 설치기의 결함이 아니다 |

| 3 | https://github.com/115dkk/Open-Mobile-Emulator/actions/runs/36826094020 (06:41~07:03 UTC, 22분) | **통과.** SourceForge가 06:41 UTC에 돌아온 것(206)을 감시가 확인하고 띄웠다. 드라이버 결과 `needs-review`(전 단계 통과, 실패 없음, `runner-dod-result.json`) |

세 번째 시도의 사실: 설치기 `.sha256` 대조와 `gh attestation verify --signer-workflow .../release.yml` 통과(9초). 표준 사용자
`omeuser`의 무음 설치 종료 코드 0, 설치 위치 `C:\Users\omeuser\AppData\Local\Open Mobile Emulator`, 도우미와 QEMU 있음, 현재
사용자의 제거 항목 `DisplayVersion 0.1.0`. 설치된 제품으로 돌린 마법사의 단계별 시간은 다음과 같다.

| 단계 | 시간 | 비고 |
|---|---|---|
| S1.1 호스트 점검, S1.2 WHPX | 1.8초, 1.8초 | 러너는 OpenGL이 없어 제품이 소프트웨어 렌더링으로 바꿈 |
| S1.4 이미지 내려받기 | 33.6초 | 2.4 GB, SHA-256 검증 포함 |
| S1.5 설치 | 62.7초 | `설치하기` 뒤 입력 없음(개발 PC 31초의 두 배. 4 vCPU 러너) |
| S1.6 첫 부팅 | 96.2초 | 직접 커널 부팅, 소프트웨어 렌더링 |
| S1.7 앱 설치 | 27.5초 | |
| 게임 실행 | 16.0분 | 1분에 권한 대화상자, 2분부터 게임 화면. 마지막 화면(`game-15`)은 게임 제목 화면 위의 게임 서버 점검 안내("현재 서버 점검 중입니다, 예상 완료 10/1/2026 4:00 PM")였다. 점검 안내는 게임 서버가 보낸 것이므로 설치된 제품의 게스트 네트워크는 러너에서 바깥까지 닿는다. 데이터 다운로드 안내는 점검 때문에 나오지 않았다 |

이로써 완료 기준 2의 러너 쪽도 증거가 생겼다. 설치기 한 번과 마법사의 승인 외에 사람의 입력은 없었다.
