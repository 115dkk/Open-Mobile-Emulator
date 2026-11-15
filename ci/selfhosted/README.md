# 폰에서 도는 P3 게스트 빌드 CI

이 CI는 브랜치 `selfhosted/guest-build`에만 있다. 유지보수자의 안드로이드 폰에서 P3 게스트 이미지(`guest/build/`)를
빌드한다. 결정과 근거는 main의 `docs/P3-PLAN.md` 5.1절과 `docs/evidence/P3/selfhosted-phone-20261115.md`에 있다(사용자
결정 2026-11-15). 빌드는 x86_64 에뮬레이션이라 네이티브의 20~35배 느리고, 한 번에 몇 주가 걸리는 것을 감수한다.

## 구조

- **빌드 컨테이너 `ome-build`.** x86_64 Ubuntu 22.04이고, proot가 x86_64 프로그램마다 qemu-user를 띄운다. main의
  `guest/build/setup-build-host.sh`로 꾸린 빌드 호스트이며 트리는 `/home/ome/bliss`에 있다.
- **러너 컨테이너 `ome-runner`.** arm64 Ubuntu 24.04에 GitHub Actions 러너가 둘 있다. `main`(라벨 `ome-guest-build`)은
  오래 걸리는 단계를, `ctl`(라벨 `ome-guest-build-ctl`)은 `status`와 `stop`을 받는다. 빌드가 `main`을 며칠씩 쥐고 있어도
  상태를 보거나 멈출 수 있게 둘로 나눴다.
- **중계 `bridge.sh`.** Termux에서 돈다. 이 폰에서는 proot 안에서 다른 proot를 띄울 수 없으므로, 러너의 작업은
  `~/ome-ci/spool`에 요청 파일을 쓰기만 하고 단계는 중계가 빌드 컨테이너에서 돌린다.
- **단계는 작업보다 오래 산다.** 셀프 호스트 러너의 작업은 5일에서 끊기고, 취소나 끊긴 연결로도 끝난다. 그래도 단계는 폰에서
  계속 돈다. 같은 단계를 다시 실행하면 돌던 단계에 붙어 로그를 이어 보여 준다. 단계는 한 번에 하나만 돈다.

## 보안

공개 저장소의 셀프 호스트 러너에는 포크 PR의 코드가 돌 수 있어 GitHub가 쓰지 말라고 경고한다. 이 CI는 그 위험을 이렇게 막는다.

1. 워크플로는 이 브랜치에만 있고 수동 실행(`workflow_dispatch`)만 받는다. GitHub는 한 번도 실행된 적 없는 워크플로를 기본
   브랜치 밖에서 수동 실행하게 해 주지 않으므로, 첫 커밋에만 이 브랜치의 push 트리거를 두어 등록하고(작업은 `if`로 건너뜀)
   다음 커밋에서 뺐다.
2. 브랜치는 규칙 세트 `selfhosted/guest-build`가 지킨다. 갱신, 삭제, 강제 푸시를 막고 저장소 관리자(115dkk)만 우회한다.
   115dkk는 PR 없이 바로 푸시할 수 있다.
3. 러너의 작업 시작 훅(`job-started.sh`)이 모든 작업을 첫 단계 전에 거른다. 저장소, 이벤트, 브랜치, 워크플로 파일
   (`GITHUB_WORKFLOW_REF`), 실행한 사람(`GITHUB_ACTOR`, `GITHUB_TRIGGERING_ACTOR`, 이벤트의 `sender`)이 모두 맞아야 하고,
   `ctl` 러너는 `status`와 `stop`만, `main` 러너는 나머지 단계만 받는다. 이 검사는 폰에 있어서 워크플로 파일로는 바꿀 수 없다.
   포크 PR이 자기 워크플로에 이 라벨을 적어도 여기서 끝난다.
4. 포크 PR의 워크플로는 모두 관리자의 승인을 거친다(저장소 설정 `all_external_contributors`).

그리고 워크플로의 `GITHUB_TOKEN`에는 권한이 없다(`permissions: {}`). 단계 이름과 `ref`는 중계가 다시 검사한다.

## 쓰는 법

Termux에서 이렇게 쓴다.

```sh
ome-ci start              # 중계와 러너 둘을 tmux 세션 ome-ci에서 띄운다(wake lock도 잡는다)
ome-ci run build          # GitHub에서 build 단계를 실행한다(gh workflow run)
ome-ci run status         # 지금 도는 단계와 로그 끝, 폰의 저장 공간과 메모리
ome-ci status             # 같은 내용을 GitHub를 거치지 않고 본다
ome-ci log                # 지금 도는 단계의 로그를 따라간다
ome-ci stop-stage         # 도는 단계를 멈춘다(ome-ci run stop과 같다)
ome-ci stop               # 서비스를 멈춘다. 도는 단계는 계속 돈다
```

이 워크플로는 기본 브랜치에 없어서 Actions 화면에 'Run workflow' 단추가 없다. 다른 PC에서는
`gh workflow run guest-build-selfhosted.yml -R 115dkk/Open-Mobile-Emulator --ref selfhosted/guest-build -f stage=build`로
실행한다. 실행 기록과 로그는 Actions 화면에 그대로 남는다.

## 단계

| 단계 | 하는 일 | 걸리는 시간 |
|---|---|---|
| `status` | 지금 도는 단계(없으면 마지막 단계)와 로그 끝 60줄, 폰의 저장 공간과 메모리 | 바로 |
| `stop` | 도는 단계를 멈춘다. 빌드 컨테이너의 프로세스를 모두 끝낸다. 트리에는 그때까지 빌드한 것이 남는다 | 바로 |
| `init` | `repo init`(스냅숏 매니페스트) | 몇 분 |
| `sync` | `repo sync`, 약 100 GB | 10~20시간으로 추정. 내려받는 시간은 와이파이에 따라 따로 든다. git의 팩 풀기(`index-pack`)가 에뮬레이션에서 약 9배 느리다(24 MB 팩 0.66초 대 5.76초) |
| `aaropa` | 설치 환경 파일을 받아 체크섬 확인 | 몇 분 |
| `build` | `make iso_img`, 메모리 8 GB(`OME_MEM_BUDGET_GB`), `-j2` | 몇 주(추정) |
| `collect` | ISO와 체크섬, build.prop, 로그를 `/home/ome/bliss/dist/`로 | 몇 분 |
| `all` | init, sync, snapshot, aaropa, build, collect를 차례로 | 몇 주 |
| `host-setup` | 빌드 호스트를 다시 꾸린다(`build-host-prepare.sh`). pins가 바뀌었을 때 | 한 시간 남짓 |

`ref` 입력은 빌드 스크립트(`guest/build`, `manifests`)를 가져올 브랜치, 태그, 커밋이고 기본은 `main`이다. 단계마다
`~/ome-ci/src`가 그 `ref`를 새로 받으므로 이 브랜치는 main을 따라갈 필요가 없다. 저메모리 빌드(`OME_MEM_BUDGET_GB`)가
없는 옛 스크립트로는 빌드 단계를 거부한다. 처음에는 `init`, `sync`, `aaropa`, `build`, `collect` 순서로 돌리거나 `all`을 쓴다.
멈춘 단계는 같은 단계를 다시 실행하면 이어서 한다(sync는 받은 것을 건너뛰고, ninja는 빌드한 것을 건너뛴다).

## 처음 준비

이 브랜치를 클론한 Termux에서 한다. 두 명령 모두 다시 돌려도 된다.

1. `bash ci/selfhosted/setup-phone.sh install`: Termux 패키지, 스크립트(`~/ome-ci/bin`, `~/.local/bin/ome-ci`), 두 컨테이너,
   빌드 호스트, 러너 둘.
2. `bash ci/selfhosted/setup-phone.sh register`: gh로 등록 토큰을 받아 러너 둘을 등록하고, 각 러너의 `.env`에 스풀 위치와
   역할, 작업 시작 훅을 적는다.
3. `ome-ci start`.

## 폰에서 해 둘 것

- 충전기에 연결해 둔다.
- Termux 알림을 지우지 않는다. `ome-ci start`가 wake lock을 잡는다. 배터리 최적화에서 Termux를 뺀다.
- 개발자 옵션에서 자식 프로세스 제한을 끈다(Android 14부터 있는 'Disable child process restrictions'). 켜 두면
  안드로이드가 Termux의 자식 프로세스를 정리하다 빌드를 죽일 수 있다.
- 폰이 재부팅되거나 Termux가 죽으면 `ome-ci start`를 하고 같은 단계를 다시 실행한다. 자동 시작(Termux:Boot)은 아직 없다.

## 네트워크가 잠깐 끊길 때

- 러너는 끊긴 동안 다시 연결을 시도하고, 러너 프로세스가 끝나면 `ome-ci`가 15초 뒤 다시 띄운다.
- 단계는 폰에서 돌므로 GitHub와의 연결이 끊겨도 멈추지 않는다. 그 사이 작업이 끝나면 같은 단계를 다시 실행해 붙는다.
- 스크립트 받기(`git fetch`, 체크아웃)는 중계가, `repo init`과 `sync`와 설치 환경 파일 내려받기는 `build-image.sh`가 재시도한다
  (`OME_NET_ATTEMPTS`, `OME_SYNC_ATTEMPTS`). 내려받기는 끊긴 곳부터 이어 받는다. 빌드 자체는 네트워크를 쓰지 않는다.

## 메모리

빌드 단계는 `OME_MEM_BUDGET_GB=8`, `OME_BUILD_JOBS=2`로 돈다(`pins.env`). 폰의 RAM 15 GB 가운데 나머지는 안드로이드
몫이다. 동시에 도는 작업 수는 이것으로 묶이지만 `soong_build`의 분석, kati, 빌드 그래프를 읽는 ninja는 프로세스 하나가 따로
8 GB를 넘을 수 있고, 그때는 폰의 스왑(20 GB, 종류는 확인 못 함)에 기댄다. 자세한 내용은 main의 `guest/build/README.md`
"Low memory"에 있다.

## 알려진 한계

- glibc 정적 PIE 프로그램은 이 폰의 qemu-user에서 죽는다. Ubuntu의 `ldconfig`는 `build-host-prepare.sh`가 아무것도 하지
  않는 스크립트로 바꿨다. 빌드 도중 같은 종류의 프로그램이 나오면 그 단계가 실패하므로 로그를 보고 따로 대응한다.
- `stop`은 빌드 컨테이너의 모든 프로세스를 끝낸다. 그 컨테이너에서 손으로 셸을 열어 두었다면 그것도 끝난다.
- 걸리는 시간은 hello 컴파일과 `index-pack`으로 잰 배율에서 어림한 값이다. 첫 빌드가 끝나면 실제 시간을 증거 문서에 적는다.
- `init`과 `sync`는 아키텍처와 무관하므로 Termux에서 네이티브로 같은 트리에 돌리면 더 빠를 수 있다. 아직 그렇게 만들지 않았다.

## 파일

| 파일 | 어디서 | 하는 일 |
|---|---|---|
| `pins.env` | 모두 | 저장소와 브랜치, 라벨, 컨테이너 이미지 다이제스트, 러너 판과 체크섬, 빌드 설정 |
| `lib.sh` | Termux | 공용 함수(재시도, ref 검사, 소스 받기, 안드로이드 변수를 뺀 빌드 컨테이너 진입) |
| `setup-phone.sh` | Termux | 처음 준비와 러너 등록 |
| `ome-ci` | Termux | 서비스 시작과 중지, 상태, 로그, 실행 |
| `bridge.sh` | Termux | 요청을 받아 단계를 시작하거나 붙이거나 거절, `status`와 `stop` |
| `run-task.sh` | Termux | 단계 하나: 스크립트를 받고 빌드 컨테이너에서 돌린 뒤 종료 코드를 남긴다 |
| `build-host-prepare.sh` | 빌드 컨테이너 | 에뮬레이션에 필요한 손질 뒤 `setup-build-host.sh` |
| `job-started.sh` | 러너 컨테이너 | 작업 시작 훅(보안 3번) |
| `request.sh` | 러너 컨테이너 | 워크플로의 유일한 단계: 요청을 쓰고 로그를 따라간다 |
| `../../.github/workflows/guest-build-selfhosted.yml` | GitHub | 수동 실행 전용 워크플로 |
