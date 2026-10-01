# ADR-0011: 설치기는 현재 사용자 범위의 NSIS이고, 릴리스는 태그에서 러너가 만든다

상태: 채택, 2026-10-01. 설치 범위와 첫 릴리스의 표시, M3 완료 기준은 사용자 결정이다(같은 날).

## 배경

M2가 끝나 제품은 마법사만으로 게임까지 간다(`docs/evidence/M2/dod-local.md` 33회차). M3는 그것을 다른 PC에
설치할 수 있는 모양으로 내보내는 일이다. CLAUDE.md M3와 ADR-0007이 정한 틀은 다음과 같다. 설치 경로에는 QEMU
바이너리, OVMF, 제품만 들어가고 게스트 이미지는 들어가지 않는다(R2). 릴리스에 올리는 실행 파일과 R4 소스 묶음은 태그
커밋에서 GitHub Actions 윈도우 러너로만 빌드하고 파일마다 빌드 출처 증명을 붙인다. 코드 서명은 하지 않는다(8절 3번).

아직 정하지 않았던 것이 넷이다. 설치기의 종류와 설치 범위, 커스텀 QEMU를 어떻게 설치기에 넣을지, QEMU 빌드를 제품
릴리스마다 러너에서 다시 할지, 그리고 M3의 완료 기준이다.

## 결정

1. **설치기는 Tauri 번들러의 NSIS 설치기이고 설치 범위는 현재 사용자다.** 프런트엔드가 Tauri 2이므로(D7) 번들러가
   이미 있다. MSIX는 서명이 없으면 설치되지 않으므로 뺀다. 범위는 사용자가 현재 사용자로 정했다(2026-10-01). 서명이
   없는 설치기를 모든 사용자 범위로 두면 설치할 때마다 UAC 창에 "알 수 없는 게시자"가 뜨는데, 현재 사용자 범위는 UAC
   없이 `%LOCALAPPDATA%\Open Mobile Emulator`에 설치하므로 경고가 내려받은 파일을 처음 열 때의 SmartScreen 한 번으로
   준다. WHPX 켜기는 지금처럼 그때만 `ome-setup.exe`로 승격한다. 제품 데이터는 그대로
   `%LOCALAPPDATA%\OpenMobileEmulator`에 있어 설치 폴더와 섞이지 않고, 제거해도 남는다.
2. **설치 폴더를 여는 단추를 둔다.** 사용자가 지적한 대로 `%LOCALAPPDATA%` 아래는 사용자가 찾아가기 어렵다. 설정의
   `정보` 절에 `프로그램 폴더 열기` 단추를 두고 런타임 명령 `open_install_folder`가 실행 파일이 있는 폴더를 탐색기로
   연다. 사용자의 다른 프로젝트(EqualizerAPO-XT의 `프로그램 폴더 열기` 메뉴)와 같은 방식이다.
3. **QEMU와 도우미는 설치기 안에 든다.** 릴리스 빌드 때만 쓰는 설정 덧판 `ci/release/tauri.release.conf.json`이
   `host/qemu-dist/`(러너가 QEMU 릴리스 zip을 푼 자리)를 `qemu/`로, `host/target/release/ome-setup.exe`를 설치 폴더
   루트로 동봉한다. `host/app/tauri.conf.json`에는 넣지 않는다. tauri-build는 설정의 자원을 매 빌드마다 대상 폴더로
   복사하므로 거기에 넣으면 개발 빌드와 CI 빌드마다 437 MB를 복사하고 자리가 비어 있으면 빌드가 실패한다. 제품의
   호스트 조사는 실행 파일 옆 `qemu/bin`을 "product-bundle"로 찾으므로 설치된 배치는 개발 트리(`host/target/debug/qemu`)와
   같다. Tauri의 자원 맵은 글롭이면 구조를 펴 버리고 폴더면 구조를 보존한다(tauri-utils 2.9.3 `resources.rs`). 그래서
   폴더를 적었다.
4. **QEMU는 따로 빌드하고 제품 릴리스는 그 결과를 검증해 가져온다.** 워크플로 `qemu-release.yml`(잡 이름
   `qemu-source-offer`, R4)이 MSYS2 UCRT64 러너에서 `qemu-build/build-qemu.sh all`을 돌려 런타임 zip, R4 소스 묶음,
   SHA256SUMS를 만들고 출처 증명을 붙여 `qemu-<QEMU 태그>-<접미사>` 사전 릴리스로 올린다. 제품 릴리스
   `release.yml`은 `manifests/qemu-release.json`이 가리키는 그 릴리스의 두 파일을 받아 SHA-256을 대조하고 `gh attestation
   verify --signer-workflow .../qemu-release.yml`로 출처를 확인한 뒤 zip을 설치기에 넣고 소스 묶음을 같은 릴리스에
   다시 올린다. 이 검증 잡도 `qemu-source-offer`라 불리고 실패하면 릴리스가 없다. QEMU 빌드는 `qemu-build/`가 바뀔
   때만 필요하고 러너에서 한 시간 가까이 걸리므로 제품 릴리스마다 되풀이하지 않는다. 출처는 끊기지 않는다. 설치기의
   증명은 제품 워크플로와 태그 커밋을, 동봉된 QEMU의 증명은 QEMU 워크플로와 그 커밋을 가리키며, 제품 워크플로 로그에
   그 검증이 남는다.
5. **릴리스 자산의 이름과 집합은 고정이다.** 설치기는 `Open-Mobile-Emulator-<버전>-x64-setup.exe` 하나뿐이고(제품의
   업데이트 확인은 exe 자산이 둘이면 거부한다. GitHub는 자산 이름의 공백을 바꾸므로 제품 이름의 공백을 뺐다) 옆에
   같은 이름의 `.sha256`이 있다. 그리고 QEMU 런타임 zip, 소스 묶음 tar.gz, `THIRD_PARTY.md`, `NOTICE`, `LICENSE`,
   `SHA256SUMS`다. `ci/Check-ReleaseAssets.ps1`이 올리기 전에 이 집합을 검사한다(M3 3번).
6. **첫 릴리스 v0.1.0은 사전 릴리스로 올린다**(사용자 결정 2026-10-01). 제품의 자동 업데이트는 GitHub의 `latest`
   릴리스만 보므로 사전 릴리스끼리는 업데이트를 권하지 않는다. 정식 릴리스를 처음 올릴 때부터 권한다.
7. **설정의 `저장 위치` 절에 스크린샷 폴더와 공유 폴더 단추를 둔다**(사용자 요구 2026-10-01 오후). 사용자가 설정에서
   바로 찾고 싶은 것이 둘 더 있다. "스크린샷을 찍었는데 어디서 보지"는 이미 있는 `open_screenshots_folder`를 설정에도
   노출하면 된다(지금은 앱 화면에만 있다). "에뮬레이터에 내 데이터를 넣고 싶은데"는 기능이 없어 **공유 폴더**를
   만든다. 홈 아래 `shared` 폴더에 파일을 넣고 `가상 머신으로 보내기`를 누르면 제품이 adb로 가상 머신 내부 저장소의
   `/sdcard/OME/`에 복사한다(명령 `open_shared_folder`, `shared_push`). 한 방향뿐이고 실시간 마운트가 아니다. 윈도우
   호스트의 QEMU에는 9p와 virtiofs가 없고 QEMU 내장 SMB는 호스트에 Samba를 요구하므로, 지금 윈도우에서 쓸 수 있는
   길은 adb뿐이다. 영상 녹화 기능은 없으므로 영상 폴더도 없다.
8. **설치 범위 선택(`both`)은 v0.1.0에 넣지 않는다.** 사용자는 기본을 현재 사용자로 두되 모든 사용자 범위도 고를 수
   있게 해도 된다고 했다(2026-10-01 오후, "굳이 한쪽으로만 적용할 필요가 없다면"). Tauri의 NSIS 틀에서 `both`는
   `MULTIUSER_EXECUTIONLEVEL Highest`와 함께 가고 기본 범위를 현재 사용자로 두는 정의가 없다. 그래서 관리자 계정에서는
   설치기를 여는 순간 UAC가 뜨고 기본 선택도 모든 사용자가 되어, 현재 사용자 범위의 장점(UAC 없음)이 사라진다. 조건이
   맞지 않으므로 지금은 현재 사용자뿐이다. 나중에 넣으려면 Tauri 틀을 복제한 사용자 정의 `.nsi`에서 실행 수준을
   표준으로 두고 모든 사용자를 고를 때만 승격하는 처리(표준 NSIS `MultiUser.nsh`에는 없고 별도 플러그인이나 직접
   재실행이 필요하다)를 더해야 한다. 그 방법은 가능하다. `RequestExecutionLevel user`로 열고, 범위 페이지에서 모든
   사용자를 고른 채 `다음`을 누를 때 `ExecShellWait "runas"`로 자신을 `/AllUsers`로 다시 띄워 그 순간에만 UAC를 받고,
   거절되면(`ERROR_CANCELLED`) 같은 페이지에 머물게 하면 된다. 다만 Tauri 틀 복제본 약 1,000줄을 저장소가 떠안아
   `@tauri-apps/cli`를 올릴 때마다 상류 변경을 옮겨야 하고, 승인과 거절 두 경우의 시험은 사람이 UAC를 눌러야 한다.
   사용자는 이 비용을 듣고 현재 사용자 범위만 두기로 다시 정했다(2026-10-01 오후).
9. **M3 완료 기준**(사용자 승인 2026-10-01). (1) 태그 커밋에서 릴리스 워크플로가 설치기, QEMU 묶음, 소스 묶음,
   SHA256SUMS를 만들고 전부 출처 증명을 붙여 올린다. (2) 그 설치기로 깨끗한 계정에 설치해 마법사만으로 게임 실행까지
   간다(완료 실행 드라이버를 설치된 제품에 대고 돌린다. 개발 PC의 새 홈과 GitHub 러너 둘 다). (3) KNOWN_LIMITATIONS
   확정, 릴리스 노트, `docs/help/`의 SmartScreen과 검증 절차. (4) 릴리스에 소스 묶음, THIRD_PARTY.md, NOTICE가 들었는지
   CI가 검사한다.

## 결과

- `installer/` 디렉터리는 만들지 않는다. 설치기의 정의는 `host/app/tauri.conf.json`(NSIS, 현재 사용자, 한국어와
  영어, WebView2 설치 건너뜀)과 `ci/release/`에 있다.
- 설치기 크기는 QEMU 런타임(437 MB, 그중 펌웨어와 데이터가 317 MB)이 정한다. 다른 아키텍처의 펌웨어를 덜어내면
  줄겠지만 `build-qemu.sh`가 설치 데이터를 통째로 보존하기로 한 이유(ROM 누락 방지)가 있어 v0.1.0에서는 건드리지
  않는다.
- 제거는 설치기가 등록한 제거 항목(현재 사용자의 `Uninstall` 키)으로 하며, 설치 폴더의 파일만 지운다. 게스트 디스크와
  로그는 `%LOCALAPPDATA%\OpenMobileEmulator`에 남는다.
- 러너의 MSYS2 저장소는 구르는 저장소라 `pins.env`의 ANGLE 고정 버전(`2.1.r25748.890b5d8f-6`)이 사라지면
  `qemu-release.yml`의 deps 단계가 실패한다. 그때는 고정을 새 버전으로 옮기고 다시 빌드한다. 패키지 버전은 기록할 뿐
  얼리지 않는다는 `qemu-build/README.md`의 원칙 그대로다.
- `m2-dod.yml`은 여전히 개발 PC에서 만든 `qemu-v11.1.1-ome3`를 받는다. 러너가 만든 QEMU 릴리스가 생기면 그 zip과
  SHA-256으로 바꾼다.
