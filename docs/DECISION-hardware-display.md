# 가상 머신 크기, 해상도, 주사율, 수직 동기화 (사용자 요구 2026-09-27)

사용자 요구 둘. "추가 하드웨어를 더 잡을 수 있는지 궁금합니다. 32기가, 16vCPU 같은 형태의 지원이
불가능하다면 그 이유를 설명해주세요. 아니라면 지원하세요." 그리고 "설정에 고해상도, 고주사율
언락을 지원할 수 있는지 알아봅시다. 트릭컬에는 실익이 없지만 필요한 게임이 있을 수 있습니다.
그와 별개로 수직동기화 계열도 지원해야 합니다."

답은 둘 다 "지원한다"이고, 이 문서는 근거와 한계와 구현 위치를 적는다. 웹 조사는 ASTRA 워커가
2026-09-27에 했고(URL은 그날 접근), 실험은 같은 날 이 PC에서 했다(`docs/evidence/M2/sizing-20260927/`).

## 1. 가상 머신 크기: 32 GB, 16 vCPU

### 답

지원한다. 막을 이유가 없고 실제로 부팅했다. 제품의 상한은 고정값이 아니라 호스트에서 읽는다.

### 사실

- QEMU 11.1.1의 WHPX 가속기는 `-smp` 값을 그대로 파티션의 프로세서 수로 넘긴다
  (`target/i386/whpx/whpx-all.c`: `prop.ProcessorCount = ms->smp.cpus`). 자체 상한은 없다. q35
  머신의 상한은 4096이다(`hw/i386/pc_q35.c`). 마이크로소프트의 WHP API 문서는 프로세서 수의
  숫자 상한을 적지 않고, Windows 11 Hyper-V 문서는 "32 프로세서와 512 GB RAM의 큰 가상 머신"을
  가능한 구성으로 적었다(https://learn.microsoft.com/en-us/virtualization/api/hypervisor-platform/funcs/whvpartitionpropertydatatypes,
  https://raw.githubusercontent.com/MicrosoftDocs/Virtualization-Documentation/35e20bab8ffb6f240bf7a28554a4d7c1bdbb6d27/virtualization/hyper-v-on-windows/reference/hyper-v-requirements.md).
- 실험(부팅 A): 16 vCPU, 32,768 MiB로 25초 만에 부팅. 게스트 `nproc` 16, `MemTotal` 32.9 GB. 호스트
  QEMU 프로세스의 개인 메모리 33.4 GiB. 게스트가 뜬 뒤 호스트 여유 42 GB / 64 GB.
- 메모리는 QEMU가 호스트에서 전부 할당한다. 그러므로 게스트 메모리 + 호스트가 쓸 예비분이
  물리 메모리 안에 들어야 한다.
- 다른 제품: LDPlayer는 "CPU 코어 수는 호스트와 같게, 메모리는 호스트의 절반까지"를 안내한다
  (2019, https://www.ldplayer.net/support/ways-to-configure-ram-and-cpu-assignment.html). BlueStacks 5는
  12 GB까지이고 호스트 16 GB 초과를 요구한다(https://support.bluestacks.com/api/v2/help_center/en-us/articles/360057205611.json).
- 변환기가 ARM 코드에 보여 주는 `/proc/cpuinfo`는 2코어다(`docs/evidence/M1/cpuinfo-experiment.txt`,
  `KNOWN_LIMITATIONS.md`). 그래서 게임의 스레드 수는 vCPU를 늘려도 그대로다. vCPU를 더 주면
  운영체제와 다른 앱, 변환기 자체의 여유가 늘 뿐 게임 프레임이 그만큼 오르지는 않는다. 이 사실은
  화면에 적지 않고 여기와 `KNOWN_LIMITATIONS.md`에만 둔다.

### 설계

- `SettingsView.memory_mib_max` = 호스트 물리 메모리 − 4096 MiB를 1024 단위로 내림(최소 4096),
  `vcpus_max` = 호스트 논리 프로세서 수(최소 2). 기본값 8192 MiB, 4 vCPU 그대로. 슬라이더의 눈금은
  이 값까지 이어진다(`ARCHITECTURE.md` 8.5절, `HostProbe::total_memory_bytes`, `logical_processors`).
- 게스트 설정 검증기(`ome-guest-config`)는 런처와 같은 넓은 범위(128~1,048,576 MiB, 1~1024 vCPU)를
  유지하고 제품 한도는 설정 층에 있다.

## 2. 해상도

### 답

지원한다. 프리셋 셋에 사용자 지정(640~7680, 8의 배수, DPI 120~640)을 더한다. 검증된 것은
1280x800과 1920x1080(M0, M1)이고 4K는 미검증으로 적는다.

### 사실

- `edid=off`인 지금도 게스트의 DRM 모드 목록에는 1280x800부터 4096x2160까지 35개 모드가 있다
  (부팅 A). 게스트 virtio-gpu 드라이버의 상한은 8192x8192(리눅스 v6.1 `virtgpu_display.c`
  `XRES_MAX`), SDL2 창 상한은 16384x16384, QEMU EDID 생성기의 목록에는 3840x2160 @ 50/60 Hz가
  있다(https://raw.githubusercontent.com/torvalds/linux/v6.1/drivers/gpu/drm/virtio/virtgpu_display.c,
  https://wiki.libsdl.org/SDL2/SDL_CreateWindow, https://raw.githubusercontent.com/qemu/qemu/v11.1.0/hw/display/edid-generate.c).
- 안드로이드 13의 `wm size`는 물리 크기의 2배까지만 받는다(`DisplayContent.setForcedSize`,
  https://raw.githubusercontent.com/aosp-mirror/platform_frameworks_base/android-13.0.0_r1/services/core/java/com/android/server/wm/DisplayContent.java).
  그러므로 물리 모드가 1280x800이면 `wm size`만으로 3840x2160은 되지 않고, 물리 모드 자체를 바꿔야
  한다. 물리 모드는 부팅 인자 `video=WxH`(Bliss 문서의 방법, https://docs.blissos.org/installation/install-in-a-virtual-machine/install-in-qemu/)와
  장치의 `xres`, `yres`(EDID 선호 모드)로 정한다. `wm size`와 `wm density`는 설정에 저장되어
  재부팅 뒤에도 남는다(`DisplayWindowSettings`).
- 호스트 GPU의 최대 텍스처 크기(`GL_MAX_TEXTURE_SIZE`)는 재지 않았다. 4K는 virgl 성능과 함께
  검증 항목이다.

### 설계

- `display_custom_apply { size, density_dpi }`는 (1) 이미지 프로필의 부팅 인자 `video=`를 갱신하고
  (2) 장치의 `xres`, `yres`를 그 값으로 두며(EDID를 켤 때) (3) 실행 중이면 `wm size`, `wm density`를
  적용한다. 물리 모드가 바뀌는 적용은 `다시 시작해야 적용됩니다.`, 크기와 밀도만 바뀌는 적용은
  `즉시 적용할 수 있습니다.`(`M2-SCREENS.md` 5절).

## 3. 주사율

### 답

지원한다. QEMU 11.1의 virtio-gpu는 주사율을 장치 속성으로 받지 않으므로 자체 패치가 필요하고,
그 패치를 이 저장소에 넣었다(`qemu-build/patches/0002-virtio-gpu-refresh-rate-property.patch`, R4의
소스 묶음에 포함). 패치된 QEMU로 게스트가 1280x800 @ 119.997 Hz로 부팅하고 SurfaceFlinger VSYNC
주기가 8.33 ms가 되는 것을 확인했다(부팅 D, `docs/evidence/M2/sizing-20260927/README.md`). 안드로이드
쪽 설정은 필요 없었다. 게임이 그 주기로 실제로 그리는지는 게임과 GPU에 달렸고 게임 세션에서
따로 잰다.

### 사실

- virtio-gpu의 EDID 주사율은 장치 속성이 아니다. `refresh_rate` 속성은 `DEFINE_EDID_PROPERTIES`를 쓰는
  `VGA`, `bochs-display`에만 있고(QEMU 커밋 `fce39fa7`, 2021-04-27, 단위 mHz), virtio-gpu는 디스플레이
  백엔드가 보내는 `QemuUIInfo.refresh_rate`를 EDID에 넣는다(`hw/display/virtio-gpu-base.c` 111행).
  GTK 백엔드는 호스트 모니터의 주사율을 보내고(`ui/gtk.c` 806행), SDL 백엔드는 보내지 않아 생성기
  기본값 75 Hz가 된다(`hw/display/edid-generate.c` 390행). 그래서 M0에서 75 Hz 단일 모드가 나왔고
  런처가 `edid=off`로 돌아갔다(`KNOWN_LIMITATIONS.md`).
- 안드로이드는 HWC가 알려 주는 모드의 주사율로 VSYNC 주기를 정한다. Bliss의 drm_hwcomposer 계열은
  DRM 모드의 `v_refresh`에서 `VsyncPeriod`를 계산한다(https://raw.githubusercontent.com/ARM-software/drm-hwcomposer/master/drmhwctwo.cpp).
  측정: 75 Hz EDID에서 13.3 ms(M0), 60 Hz 모드에서 16.678 ms(부팅 A). 그러므로 EDID에 120 Hz를 주면
  VSYNC가 8.33 ms가 될 것이고, 게임이 그 속도로 그리는지는 게임과 GPU에 달렸다.
- 호스트 쪽 표시는 SDL의 스왑 간격 0(즉시)이라 게스트가 그린 만큼 바로 창에 올라간다. 호스트
  모니터 주사율보다 높은 게스트 주사율은 모니터에 보이지 않지만 입력 지연은 줄 수 있다.

### 설계

- 장치 속성 `refresh_rate`(mHz, 30000~240000)가 EDID의 선호 모드에 실린다. 첫 시도였던 SDL 옵션
  방식은 SDL 창의 시작 크기가 장치의 `xres`, `yres`를 덮어써 폐기했다(부팅 C). 제품은
  `DisplayView.refresh_rate_hz`가 None이면 `edid=off`(지금과 같음), Some이면
  `virtio-vga-gl,edid=on,xres=,yres=,refresh_rate=<hz*1000>`을 낸다. `refresh_supported`는 QEMU 번들의
  `ome-patches.txt`에 0002가 있을 때 참이고, 그때만 주사율 행이 화면에 있다(`ARCHITECTURE.md` 8.4절).

## 4. 수직 동기화

### 답

지원한다. QEMU 11.1의 SDL GL 경로는 `SDL_GL_SetSwapInterval(0)`으로 고정되어 있어
(`ui/sdl2.c` 124행) 옵션으로 여는 자체 패치가 필요하고, 그 패치를 넣는다
(`qemu-build/patches/0003-ui-sdl2-swap-interval-option.patch`).

### 설계

- `-display sdl,swap-interval=<-1|0|1>`. 0이 지금의 동작(즉시), 1이 호스트 수직 동기화(호스트
  모니터 주사율에 맞춰 대기, 찢어짐 없음), -1이 적응형(SDL이 지원할 때, 아니면 0으로 내려가고
  경고를 남긴다). 화면의 `끔 / 켬 / 적응형`에 대응하고 `vsync_supported`는 0003의 존재로 판단한다.
- 수직 동기화를 켜면 게스트가 호스트 모니터 주사율보다 빨리 그려도 창에는 모니터 주사율만큼만
  올라간다. 게스트 쪽 VSYNC(3절)와는 별개의 축이다.

## 5. 확인 항목

- 120 Hz 모드에서의 게임 fps와 프레임 간격(부팅 D는 모드와 VSYNC 주기까지만 쟀다).
- `swap-interval=1`과 `-1`의 호스트 표시 효과(찢어짐, 입력 지연)를 게임 세션에서 비교.
- 4K(3840x2160) 물리 모드에서 virgl 프레임과 호스트 GPU 텍스처 상한.
