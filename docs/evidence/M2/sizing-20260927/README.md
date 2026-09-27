# 가상 머신 크기와 표시 모드 실험 (2026-09-27)

호스트: `docs/evidence/M0/environment.md`의 PC(i5-12600KF 16 스레드, 64 GB, RTX 2080 SUPER,
Windows 11 22621, 모니터 3840x2160 59 Hz). 게스트: `%LOCALAPPDATA%\OpenMobileEmulator\vm\default`
(Bliss 16.9.7 GApps 2024-10-11, 안드로이드 13). 스크립트는 세션 스크래치의
`sizing-experiment2.ps1`이며 `launcher/Start-Guest.ps1 -Gpu virgl -MemoryMB 32768 -Smp 16`으로
부팅했다. 원문은 이 폴더의 `bootA-guest.txt`, `bootA-host.txt`, `experiment.log`.

## 부팅 A: 16 vCPU, 32 GiB, virgl (커스텀 QEMU 11.1.1)

| 항목 | 값 | 출처 |
|---|---|---|
| `sys.boot_completed`까지 | 25 s | `experiment.log` 10:59:07 |
| 게스트 `nproc`, `/proc/cpuinfo` processor 수 | 16, 16 | `bootA-guest.txt` |
| 게스트 `MemTotal` | 32,853,180 kB (약 31.3 GiB, 나머지는 펌웨어와 예약) | `bootA-guest.txt` |
| 호스트 QEMU 프로세스 | PrivateMemory 33,451 MiB, WorkingSet 5,225 MiB, 스레드 102 | `bootA-host.txt` |
| 게스트가 뜬 뒤 호스트 여유 메모리 | 42,091 MiB / 65,316 MiB | `bootA-host.txt` |
| 호스트 GL 경로 | `libvirglrenderer-1.dll` → `libepoxy-0.dll` → `OPENGL32.DLL` → `nvoglv64.dll`(NVIDIA 데스크톱 OpenGL). ANGLE `libEGL.dll`, `libGLESv2.dll`도 적재됨(SDL GL 경로의 EGL 호출용) | `bootA-host.txt` |

메모리는 QEMU 프로세스의 개인 메모리로 전부 잡힌다(33.4 GiB). 그러므로 제품의 상한은 호스트
물리 메모리에서 예비분을 뺀 값이어야 하고, 이 실험은 그 규칙(`ARCHITECTURE.md` 8.5절, 물리
메모리 − 4 GiB)의 근거다. 첫 시도(`experiment.log` 10:39, 스크립트 1판)는 adb가 PATH에 없어
부팅 완료를 못 읽었을 뿐 같은 크기로 부팅했고(`bootA-host.txt` 첫 블록, pid 15940), 그때는
강제 종료했다.

### 표시 모드 (`edid=off`, 지금의 런처 기본값)

`/sys/class/drm/card0-Virtual-1/modes`에 1280x800부터 4096x2160까지 35개 모드가 있고,
`dumpsys display`의 `supportedModes`는 모두 56.25~60.32 Hz다. 활성 모드는 1280x800 @ 59.96 Hz,
SurfaceFlinger `VSYNC period: 16678174 ns`. 즉 `edid=off`는 해상도 목록을 잃게 하지 않는다
(`KNOWN_LIMITATIONS.md`의 이전 문장은 이 결과로 고쳤다). 잃는 것은 EDID의 선호 모드와 주사율
정보뿐이다. `wm size`는 `Physical size: 1280x800`, `wm density` 160.

### 루트 (`adb root` 뒤)

| 항목 | 값 |
|---|---|
| `adb root` | 동작. `id`가 `uid=0(root) ... context=u:r:su:s0` |
| `getprop ro.adb.secure`, `ro.debuggable`, `service.adb.root` | 0, 0, 1 |
| KernelSU 관리자 앱 | 설치됨(`me.weishu.kernelsu`) |
| `/data/adb/ksu`, `/data/adb/ksud`, `which su`, `su -v` | 전부 없음(`su: inaccessible or not found`) |

즉 이 이미지에서 앱 루트(KernelSU)는 커널 쪽이 동작하지 않아 쓸 수 없고 adb 루트만 된다.
Bliss support issue #99(2024-09-11 빌드에서 같은 증상)가 2024-10-11 빌드에서도 재현된다.
자세한 뜻은 `docs/DECISION-root-adb.md`.

## 부팅 B: 실패 (스크립트 결함)

배포판 QEMU 11.1.0의 GTK 백엔드로 `edid=on`을 시험하려던 두 번째 부팅은 스크립트의 변수 이름
`$home`이 PowerShell 자동 변수와 충돌해 펌웨어 변수 파일 경로가 틀어져 QEMU가 바로 종료했다
(`bootB-qemu-stderr.txt`). 첫 시도의 부팅 B는 `-name` 값의 공백 때문에 실패했다. GTK 실험은 값이
적어(호스트 모니터가 59 Hz라 고주사율을 증명하지 못한다) 다시 하지 않았다.

## 부팅 C: 패치 0002 첫 판(SDL 옵션), 결함 발견

첫 판의 패치 0002는 `-display sdl,refresh-rate=120`으로 `QemuUIInfo.refresh_rate`를 보냈다. 결과
(`bootC-guest.txt`, 부팅 33 s): EDID에 119.99 Hz 모드가 생겨 **주사율이 게스트까지 닿는 것은
증명**되었지만, 같은 UI 정보에 실린 SDL 창의 시작 크기 640x480이 장치의 `xres=1280,yres=800`을
덮어써(`virtio_gpu_ui_info`는 폭과 높이를 조건 없이 복사한다) 게스트가 640x480으로 부팅했고, 활성
모드는 60 Hz 대체 모드였다. 그래서 0002를 장치 속성 방식으로 다시 만들었다
(`docs/evidence/M2/qemu-display-options.md`).

부팅 C에서 함께 잰 것: clocksource는 `hyperv_clocksource_tsc_page`(WHPX가 Hyper-V로 감지되어 TSC
페이지를 쓴다. 시계 읽기에 VM 탈출이 없다), `/proc/cpuinfo` 플래그에 `avx avx2 fma bmi1 bmi2
sse4_2 popcnt f16c aes pclmulqdq`가 있다(CPU 모델 `Skylake-Client-v4`가 AVX2와 FMA를 드러낸다),
변환기 `ro.ndk_translation.version` 0.2.3, ARM 앱에 보이는 코어 수 2. 커널 6.1.112.

## 부팅 D: 패치 0002 최종판(장치 속성), 성공

`-device virtio-vga-gl,edid=on,xres=1280,yres=800,refresh_rate=120000 -display sdl,gl=on,swap-interval=1`
(`bootD-command.txt`). 결과(`bootD-guest.txt`, 부팅 34 s):

| 항목 | 값 |
|---|---|
| 활성 모드 | 1280x800, `refreshRate=119.99717`, `presentationDeadlineNanos=8333530` |
| SurfaceFlinger `VSYNC period` | 8,333,530 ns (8.33 ms) |
| `wm size` | Physical size: 1280x800 |
| `peak_refresh_rate`, `min_refresh_rate` | 설정하지 않은 상태(null)에서 이미 120 Hz. 120으로 넣어도 같고 지운 뒤에도 같다 |
| 모드 목록 | 1280x800이 첫 항목, 나머지 표준 모드는 50/60 Hz |

즉 EDID의 선호 모드가 주사율을 실어 나르므로 안드로이드 쪽 설정(`peak_refresh_rate`)은 필요
없다. `swap-interval=1`은 오류 없이 받아들여졌고(QEMU stderr에 관련 경고 없음), 호스트 쪽 표시
효과는 게임 세션에서 따로 잰다. 종료 때의 `failed to get xsave state` 세 줄은 WHPX 리셋 경로의
알려진 메시지다(`KNOWN_LIMITATIONS.md`).
