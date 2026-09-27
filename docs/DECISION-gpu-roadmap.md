# GPU 경로의 장기 방향: DirectX 12 질문에 대한 답 (2026-09-27)

사용자 질문(2026-09-27): "DirectX 12 같은 걸 장기적으로 지원할 수 있는지 알아봅시다. 최적화 여지를
대폭 열기 위해서입니다. OpenGL(심지어 불칸도 아님)로 잉여잉여하고 있어야 한다는 게 말이 안 된다는
강경한 입장입니다."

이 문서는 조사한 사실과 그로부터 나온 답을 적는다. 조사는 ASTRA 워커가 2026-09-27에 했고 URL은
그날 접근한 것이다. 호스트 쪽 사실 하나는 이 저장소의 QEMU 11.1.1 소스와 실행 중인 QEMU
프로세스에서 직접 확인했다.

## 1. 답

DirectX 12는 이 제품의 GPU 경로에서 **목적지가 아니라 백엔드 선택지 하나**다. 운영체제 안의 앱은
OpenGL ES나 Vulkan으로 그리고 DirectX를 말하지 않는다. 그 호출이 가상 GPU를 거쳐 호스트에 닿았을
때 호스트 쪽 라이브러리가 무엇으로 그리느냐가 남는 질문이고, 여기서 선택지는 다음 셋이다.

| 경로 | 게스트 API | 호스트 렌더러 | 호스트 백엔드 | 상태 |
|---|---|---|---|---|
| 지금(virgl) | GLES | virglrenderer → 데스크톱 OpenGL | NVIDIA `nvoglv64.dll`(WGL) | 동작 중. M0, M1 증거 |
| gfxstream GLES | GLES | gfxstream → EGL/GLES | ANGLE(`libGLESv2.dll`) → **D3D11** | 라이브러리는 윈도우 지원. QEMU 통합은 미확인 |
| gfxstream Vulkan | Vulkan | gfxstream → 호스트 Vulkan | NVIDIA Vulkan 드라이버(네이티브) | 라이브러리는 윈도우 지원. QEMU 통합은 미확인 |

성능의 여지를 여는 것은 세 번째 줄이다. 게스트의 Vulkan 호출이 번역 없이 호스트의 네이티브
Vulkan 드라이버로 가므로, D3D12로 다시 번역하는 것보다 빠르면 빨랐지 느리지 않다. D3D12와
Vulkan은 같은 세대의 명시적 API이고 윈도우의 NVIDIA와 AMD 드라이버는 둘 다 네이티브로 제공한다.
D3D12가 필요한 경우는 딱 하나, 호스트에 Vulkan 드라이버가 없을 때(일부 윈도우 on ARM 기기)인데
그때 쓰는 것이 Mesa의 Dozen(D3D12 위의 Vulkan)이고, 그것은 호환성 계층이지 성능 계층이 아니다.

그러므로 "OpenGL에 갇히지 않는" 길은 **gfxstream으로 옮겨 게스트 Vulkan을 호스트 Vulkan에 그대로
넘기는 것**이고, 이것은 CLAUDE.md의 출시 후 트랙 P1과 같은 일이다. D3D12는 그 위에서 저절로
얻어지는 것이 아니라 필요하지도 않다. 이 답은 사용자의 입장과 충돌하지 않는다. 사용자가 원하는
것은 최적화의 여지이고, 그 여지는 D3D12가 아니라 Vulkan 통과에 있다.

## 2. 사실

### 2.1 지금의 호스트 경로는 OpenGL이 맞다

- 2026-09-27 실험(`docs/evidence/M2/sizing-20260927/bootA-host.txt`)에서 실행 중인 커스텀
  QEMU 프로세스에 `libvirglrenderer-1.dll`, `libepoxy-0.dll`, `OPENGL32.DLL`, `nvoglv64.dll`
  (NVIDIA 데스크톱 OpenGL)이 올라와 있었다. ANGLE의 `libEGL.dll`과 `libGLESv2.dll`도 올라와
  있지만 SDL GL 경로가 `eglGetCurrentDisplay`를 부르기 때문에 필요한 것이고(`docs/evidence/M1/custom-qemu-build.md`),
  실제 렌더링은 WGL 컨텍스트 위의 NVIDIA OpenGL이다. `dxgi.dll`은 SDL 창 관리에서 올라온다.
- Trickcal은 Unity의 GLES 경로로 돈다(M0 증거). Vulkan 필수 게임은 이 경로에서 미지원이다
  (`KNOWN_LIMITATIONS.md`).

### 2.2 DirectX는 게스트에 없고, 호스트에서는 ANGLE의 백엔드로만 등장한다

- ANGLE의 백엔드 목록은 D3D9, D3D11, 데스크톱 GL, GLES, Vulkan, Metal이다. **D3D12 백엔드는 없다.**
  2023-03-17 ANGLE 개발자 Geoff Lang: "There are no current plans to support a D3D12 in ANGLE"
  (https://github.com/google/angle/blob/main/README.md,
  https://groups.google.com/g/angleproject/c/5DTHugKndHk). ANGLE이 D3D12 앱과 함께 쓰이는
  방법은 D3D11on12뿐이고 그것은 D3D12 위에서 D3D11을 흉내 내는 마이크로소프트의 계층이다.
- 구글 안드로이드 에뮬레이터는 윈도우에서 GLES를 ANGLE D3D11로, Vulkan을 호스트 GPU 드라이버로
  처리해 왔다(릴리스 노트 30.0.26, 29.0.6; https://developer.android.com/studio/releases/emulator).
  2026-02-10의 36.4.9는 호스트에서 GL을 아예 쓰지 않는 Vulkan 합성(`VulkanNativeSwapchain`)을
  더했다. 즉 구글의 방향도 D3D12가 아니라 Vulkan이다.

### 2.3 gfxstream과 rutabaga는 윈도우 호스트를 지원한다. QEMU 통합은 리눅스 전제다

- gfxstream README는 윈도우 호스트 빌드(VS 2019 + ClangCL, `gfxstream_backend`)를 문서화하고,
  테스트 실행 파일이 `libEGL.dll`, `libGLESv2.dll`(ANGLE), `vulkan-1.dll`을 필요로 한다고 적는다
  (https://github.com/google/gfxstream/blob/main/README.md). 윈도우 빌드 CI가 있다.
- rutabaga_gfx는 crosvm에서 분리된 별도 저장소가 되었고(2025-10-30), 윈도우 + gfxstream 빌드 CI와
  Win32 외부 메모리 핸들 할당기를 갖는다(https://github.com/magma-gpu/rutabaga_gfx). 유지 관리자
  Gurchetan Singh의 2025-11-18 발언: "gfxstream_vk has been working with Windows for many years now"
  (https://github.com/magma-gpu/rutabaga_gfx/issues/24#issuecomment-3548021055).
- QEMU의 `virtio-gpu-rutabaga`(8.2부터)는 문서가 리눅스 호스트(6.13+) 전제이고 윈도우 호스트
  구성은 문서에 없다(https://www.qemu.org/docs/master/system/devices/virtio/virtio-gpu.html).
  즉 라이브러리는 되고, QEMU 안에서 윈도우 호스트로 잇는 일이 P1의 실제 작업이다.

### 2.4 Venus(virglrenderer의 Vulkan)는 윈도우 호스트가 아니다

- Venus는 virglrenderer 1.0부터 있고 QEMU 11.1 문서는 리눅스 호스트 6.13+와 `hostmem`, `blob`을
  요구한다. Mesa 문서는 호스트 드라이버 요구 사항을 리눅스(`VK_KHR_external_memory_fd`)와
  안드로이드로만 적는다(https://docs.mesa3d.org/drivers/venus.html). 윈도우 호스트 지원은 없다.
- Bliss의 `typhoon-x86` 소스 구성에는 게스트 쪽 `vulkan.virtio`(Venus 게스트 드라이버)가 들어
  있고 gfxstream 자리는 주석뿐이다(https://raw.githubusercontent.com/BlissOS/device_generic_common/typhoon-x86/gpu/gpu_mesa.mk).
  설치된 16.9.7 ISO에서의 동작은 확인하지 않았다.

### 2.5 D3D12 위의 호환 계층은 성능 계층이 아니다

- Mesa Dozen은 D3D12 위의 Vulkan, Mesa d3d12 갤리움 드라이버는 D3D12 위의 OpenGL이다. 마이크로소프트는
  네이티브 드라이버가 있는 x64에서는 네이티브를 쓰라고 권하고 이 팩은 드라이버가 없는 기기용이라고
  적는다(https://devblogs.microsoft.com/directx/announcing-the-opencl-and-opengl-compatibility-pack-for-windows-10-on-arm/).
  스토어 제품 설명은 OpenGL 3.3까지, Vulkan 1.2까지를 적는다.

## 3. 로드맵

1. **지금(M2, M3)**: virgl 유지. 이 경로에서 얻을 수 있는 것은 이 저장소의 주사율과 수직 동기화
   옵션(`DECISION-hardware-display.md`)과 가상 머신 크기다.
2. **P1 (출시 후, 사용자 승인 뒤 코드)**: gfxstream. 조사 스파이크의 목표는 (a) rutabaga_gfx +
   gfxstream을 MSYS2/UCRT64 또는 MSVC로 윈도우에서 빌드하고, (b) QEMU 11.x의 `virtio-gpu-rutabaga`
   장치를 윈도우 호스트에서 컴파일해 `x-gfxstream-gles`와 `gfxstream-vulkan` capset으로 Bliss(또는
   gfxstream 게스트 드라이버가 든 자체 빌드 이미지, R3)를 띄우는 것이다. 게스트 쪽에는 gfxstream
   게스트 드라이버(`vulkan.ranchu`/gfxstream Vulkan ICD)가 필요하므로 P3(자체 이미지)과 묶인다.
   결과가 나오면 GLES는 ANGLE(D3D11), Vulkan은 네이티브 Vulkan으로 간다.
3. **D3D12는 로드맵에 없다.** 필요해지는 경우는 호스트에 Vulkan 드라이버가 없을 때뿐이고, 그때는
   Dozen을 `vulkan-1.dll` 자리에 놓는 것으로 끝난다. 별도 코드가 아니다.

## 4. 판정: Vulkan 통과 대 D3D12 백엔드 직접 작성 (2026-09-27)

사용자가 두 선택지를 제시했다. (a) Vulkan을 호스트 드라이버에 그대로 넘기고 기술 부채를 감수한다,
(b) D3D12 최신 백엔드를 직접 짠다. 판정은 (a)다.

- 게스트 앱은 D3D12를 말하지 않으므로 (b)는 GLES→D3D12나 Vulkan→D3D12 번역기를 뜻한다. 번역기는
  앱과 GPU 사이의 층 하나이고 잘해야 네이티브 Vulkan과 같다. 같은 GPU의 Vulkan과 D3D12 드라이버는
  같은 벤더가 같은 수준으로 만든다.
- 규모. ANGLE의 D3D11 백엔드와 Mesa Dozen은 벤더 팀이 여러 해를 들였고 Dozen은 아직 적합성이 완전하지
  않다(2절). ANGLE 팀은 D3D12를 만들지 않기로 했다. 이 프로젝트의 인력으로 그 유지보수를 지면 적합성
  결함이 게임 화면의 깨짐으로 나타난다.
- (a)의 부채는 통합 부채다. 윈도우 호스트용 QEMU 통합의 유지, gfxstream과 rutabaga 릴리스 추적,
  gfxstream 드라이버가 든 자체 이미지(R3). 범위가 정해져 있고 구글이 자기 에뮬레이터를 위해 윈도우에서
  계속 살려 두는 코드다.
- D3D12의 자리는 표시 단계뿐이다. DXGI 플립 모델 스왑체인, 가변 주사율, HDR을 Vulkan과 D3D12의
  상호 운용(`VK_KHR_external_memory_win32`, `VK_KHR_external_semaphore_win32`)으로 붙이는 일은 수백 줄이고
  gfxstream 뒤에 더한다.
- 통과 뒤 남는 병목은 ARM 변환기(CPU)와 virtio 전송이며 D3D12는 둘 다 건드리지 못한다.

## 5. 사용자가 정할 것

- P1의 착수 시점(CLAUDE.md 8절 6번). 이 문서로 P1의 목표는 "gfxstream 윈도우 호스트 통합"으로
  구체화되었고, 그 결과물이 Vulkan 통과다.
