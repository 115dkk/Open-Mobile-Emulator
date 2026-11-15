# P3 빌드 호스트 조사: 안드로이드 폰에서 x86_64 에뮬레이션, 2026-11-15

사용자가 2026-11-15에 정했다. P3 게스트 이미지를 유지보수자의 안드로이드 폰에서 빌드하는 셀프 호스트 CI를 두고, 메모리는
8 GB에 맞추며, 오래 걸리는 것은 감수한다. CI는 전용 브랜치 `selfhosted/guest-build`에만 두고 수동 실행만 받으며 그 브랜치는
사용자만 바꿀 수 있게 막는다. 아래는 그 결정 전후에 이 폰에서 확인한 사실이다.

## 1. 기기

| 항목 | 값 |
|---|---|
| 기기 | Samsung SM-S948N, SoC SM8850(코어 8개, `/sys/devices/system/cpu/possible` 0-7) |
| Termux 프로세스가 보는 CPU | 4개(`nproc`, Go `runtime.NumCPU`, Java `availableProcessors` 모두 4) |
| RAM | 15.4 GB(`MemTotal: 15434744 kB`) |
| 스왑 | 20 GB(`SwapTotal: 20971516 kB`). 종류(zram인지 저장소 파일인지)는 `/proc/swaps` 권한이 없어 확인하지 못했다 |
| 저장 공간 | 여유 781 GB(`/data`) |
| OS | Android 17, 커널 6.12(4K 페이지) |
| 도구 | Termux 0.118.3, proot 5.1.107.102, proot-distro 5.10.3, qemu-user-x86-64 11.1.3 |

## 2. 네이티브 arm64 빌드는 되지 않는다

고정 커밋의 soong(BlissRoms `platform_build_soong` fb17233)은 빌드 호스트가 amd64가 아니면 바로 멈춘다.
`android/arch.go`의 `config.BuildArch`가 `runtime.GOARCH`를 보고 `amd64`가 아니면 `panic("unsupported Arch")`이고,
`build/make/core/envsetup.mk`(BlissOS `platform_build_make` 04a96ca)는 `uname`에 `x86_64`가 있어야 `HOST_ARCH`를 정한다.
AOSP의 호스트 prebuilts(clang, go, jdk, build-tools)도 `linux-x86`뿐이다. 그래서 이 폰에서는 x86_64 에뮬레이션으로만 빌드할 수 있다.

## 3. 에뮬레이션에서 돌린 것

`proot-distro install -a x86_64 -n ome-build ubuntu:22.04`로 만든 컨테이너(proot가 x86_64 프로그램마다 qemu-x86_64를 띄운다)에서
`manifest/ome.xml`이 고정한 커밋의 prebuilts를 얕게 받아 돌렸다.

| 대상 | 결과 |
|---|---|
| `prebuilts/build-tools/linux-x86/bin/ninja --version` | `1.9.0.git` |
| `ckati` | 실행된다(`--version`은 없는 옵션이라는 답) |
| `toybox echo` | 정상 |
| `prebuilts/go/linux-x86` `go version` | `go1.22.1 linux/amd64` |
| 같은 Go로 hello `go run`(빈 캐시) | 1분 51초 |
| `prebuilts/jdk/jdk21/linux-x86` `java -version` | `21.0.1`, `Android_PDK` |
| `javac` hello | 15.6초 |
| `java`로 5천만 번 반복 | 4.1초 |
| soong의 javac·D8·R8 JVM 플래그에 `-XX:+UseSerialGC`를 더한 실행 | 받아들인다(`-XX:+PrintCommandLineFlags`에 `+UseSerialGC`) |
| Python 3.10(Ubuntu) | 정상 |

네이티브 대비 대략 20~35배 느리다. hello 하나를 컴파일하는 데 걸린 시간으로 어림한 값이며 빌드 전체를 잰 값은 아니다.

## 4. 에뮬레이션에서 깨진 것과 우회

- **glibc 정적 PIE.** Ubuntu의 `/sbin/ldconfig.real`(static-pie)이 qemu에서 `--version`만으로도 SIGSEGV로 죽는다. Termux의 qemu 11.1.3과
  Ubuntu 24.04의 `qemu-x86_64-static` 8.2.2가 똑같다. `-strace`를 보면 `brk`가 늘어나지 않고(요청한 주소 대신 처음 값을 돌려줌) 그 뒤
  TLS 설정에서 `0xffffffffffffffa8`을 읽다 죽는다. `-cpu` 모델, `-B`, `-R`, `setarch -R` 모두 그대로다. 컨테이너에서는
  `dpkg-divert`로 `ldconfig.real`을 아무것도 하지 않는 스크립트로 바꿔 apt가 끝나게 했다(`ld.so.cache`는 이미지에 든 것을 쓰고, 기본
  multiarch 디렉터리의 라이브러리는 캐시 없이도 찾는다). AOSP의 도구는 동적 링크이거나(ninja, ckati, toybox, JDK) brk를 쓰지 않는
  Go 정적 바이너리라 위 시험에서는 걸리지 않았다. 빌드 도중 같은 종류의 프로그램이 나오면 거기서 멈출 수 있다.
- **proot 안의 proot.** arm64 컨테이너 안에서 다른 proot를 띄우면 `/bin/true`도 종료 코드 182로 끝나고, `proot-distro`는 proot
  안에서 실행되기를 거부한다. 그래서 러너와 빌드는 따로 Termux에서 띄우는 두 컨테이너에 두고, 둘 사이를 Termux 쪽 중계
  프로세스가 잇는다.
- **x86_64 컨테이너 안의 arm64 러너.** proot는 호스트 아키텍처의 실행 파일을 안드로이드 쪽 파일 시스템 기준으로 실행하므로
  컨테이너 안의 glibc 로더를 찾지 못한다(`No such file or directory`). 러너는 arm64 컨테이너에 둔다.

## 5. 걸릴 시간(추정)

`build-20261002.md`의 4·5차 시도가 개발 PC(16 스레드, `-j12`)에서 합쳐 2시간 55분이었다. 3절의 20~35배와, 메모리 8 GB에
맞춰 동시 작업을 2개로 줄인 것을 함께 놓으면 전체 빌드는 몇 주(2~5주)로 추정한다. 잰 값이 아니라 위 두 수에서 나온 어림이다.
셀프 호스트 러너의 작업은 5일에서 끊기므로 빌드는 러너 밖의 Termux 쪽 프로세스가 돌리고, 작업은 그 빌드에 붙어 로그를 보여
줄 뿐이다(브랜치 `selfhosted/guest-build`의 `ci/selfhosted/README.md`).
