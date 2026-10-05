# 트릭컬 on Digitalis (안드로이드 15 실험 이미지), 2026-10-05

## 환경

- 이미지: `OME-api35-20261005-digitalis.iso`(SHA-256 `2e57ebdd…6bf0`), 제품 무인 설치 도우미(ADR-0010, `99-ome-install` 무수정)로
  64 GiB scratch 디스크 `%LOCALAPPDATA%\OpenMobileEmulator\p2\game64.qcow2`에 설치, `SRC=/ome` 직접 부팅(`game-disk-install.txt`).
- 제품 QEMU, WHPX, virgl(RTX 2080 SUPER, GLES 3.2 Mesa 24.3.3), `Skylake-Client-v4`, 4 vCPU, 8192 MiB, 1280x800, 59.96 Hz.
- 게임: 사용자 기기에서 2026-09-26에 뽑은 10644(base와 split 셋, 해시 대조), `install-multiple` Success, `primaryCpuAbi=arm64-v8a`.

## 결과: 첫 실행에서 SIGSEGV

| 시각(게스트) | 사건 |
|---|---|
| 13:30:40.689 | 게임 프로세스 시작 |
| 13:30:40.790 | `Initialized Berberis (aarch64)` |
| 13:30:40.958 | `libATG_D.so` 적재 성공 |
| 13:30:40.959 | `HandleFatalSignal: sig=11 si_addr=0x0` |
| 13:30:41.010 | ApplicationExitInfo `SIGNALED status=11` |

```text
E libsigchain: #03 pc 00777378 /apex/com.android.art/lib64/libart.so (art::JNI<false>::GetLongField(_JNIEnv*, _jobject*, _jfieldID*)+88)
E libsigchain: #04 pc 000086b5 /system/lib64/libberberis_proxy_libandroid.so
  (berberis::TrampolineFuncGenerator<void* (void*, void*), …>::Func(void const*, berberis::ThreadState*)+293)
E libsigchain: #05 pc 002e0bee /memfd:exec (deleted)
```

게스트 코드가 `libandroid`의 인자 둘짜리 함수를 프록시로 불렀고, 호스트 `libandroid`가 그 안에서 JNI `GetLongField`를 부르다 널 주소를 읽었다.
인자 둘(`void*, void*`)에 JNI를 부르는 `libandroid` 함수는 `AAssetManager_fromJava(JNIEnv*, jobject)`가 대표적이다(유니티가 시작할 때 부른다).
게스트의 `JNIEnv*`가 호스트용으로 바뀌지 않은 채 넘어갔을 가능성이 있다(추정, 확인 중).

함께 나온 경고: `nativebridge: Failed to bind-mount /system/etc/cpuinfo.arm64.txt as /proc/cpuinfo`. 인과는 확인하지 않았다.
게임의 거부 문구는 없었다(화면이 뜨기 전에 끝났다). 재실행과 우회는 하지 않았다.

## M0과 나란히

| 항목 | M0: libndk_translation 0.2.3, API 33, virgl | Digitalis, API 35, virgl |
|---|---|---|
| 첫 실행 | 통과 | SIGSEGV |
| 리소스 내려받기, 계정, 전투, 10분 | 통과 | 도달 못 함 |
| 전투 fps, p50/p90/p99 | 58.5, 17/20.6/28 ms | 측정 불가 |
| 게임 CPU(4 vCPU 기준), PSS | 120~140 %, 2,049 MB | 측정 불가 |

M0 값은 `docs/evidence/M0/metrics.md`의 8 GiB, 60 Hz, 전투 1-3 행이다. API 세대가 달라 번역기만 바꾼 A/B는 아니다.
원문: `game-*.txt`, `game-first-launch.png`.

## 원인 확인 (같은 날)

A15 `native_bridge_support`(`aa99591c`)의 `android_api/libandroid/proxy/trampolines_arm64_to_x86_64-inl.h`는 `AAssetManager_fromJava`를
`auto(void*, void*) -> void*`로 선언한다. 상류는 2024-11-13 커밋 `e6c71f3874270863e410dea228b75461f2199462`("Regenerate proxy libraries to use
JNIEnv explicitly")에서 생성 헤더 15개의 JNI 인자를 `JNIEnv*`로 바꿨고, Digitalis 본체는 `JNIEnv*` 인자에만 `ToHostJNIEnv` 변환을 건다
(`guest_abi/include/berberis/guest_abi/function_wrappers.h`). A15 프록시와 A16 계열 본체를 합치면 게스트 JNIEnv가 그대로 호스트로 넘어간다.

`tests/fixtures/arm64-probe`에 `AAssetManager_fromJava(env, getAssets())`만 부르는 단계를 더해 재현했다. 호출 직전 로그
`OMEProbe: AAssetManager_fromJava BEGIN` 바로 뒤에 게임과 같은 스택(`GetLongField+88`, 프록시 `+293`)이 나왔다(`jni-*.txt`).
안드로이드가 죽은 액티비티를 자동으로 다시 띄워 한 번의 실행에서 같은 충돌이 31번 기록되었고, 멈춤 기준에 따라 거기서 멈췄다.

cpuinfo 경고: A15 `libnativebridge`가 `/system/etc/cpuinfo.<isa>.txt`를 찾지만 Digitalis는 정적 파일 대신
`OpenatProcCpuinfoForGuest()`가 memfd로 ARM64 형식을 만들어 준다. 파일을 더하지 않았다.

다음: `e6c71f3`의 JNI 타입 수정 가운데 A15에 있는 심볼만 옮겨 다시 빌드, 재현기, 게임 순으로 확인.

## JNI 타입 이식 뒤 (같은 날, jni1)

상류 `e6c71f3`의 JNI 타입 변경 가운데 A15 헤더와 행이 그대로 맞는 것만 옮겼다(헤더 15개, 심볼 25개, 선언 75개, 빠진 심볼 0, `jni-fix-scope.txt`).
재현기는 `OMEProbe: AAssetManager_fromJava PASS`. 게임은 이전 충돌을 지나 `IL2CPP: JNI_OnLoad`, Unity `6000.3.13f1`, `CPU 'arm64-v8a'`,
`OS 'Android OS 15 (API 35)'`까지 갔고, 시작 2.58초 뒤 다른 SIGSEGV로 끝났다.

```text
E berberis: FATAL host signal NO-RECOVERY: depth=1 sig=11 host_pc=0x7aa4c302764c fault_addr=0x7aa4c302764c si_code=2
E libsigchain: #03 pc 0002664b .../lib/arm64/libswappywrapper.so
E libsigchain: #04 pc 0033b5a7 .../dalvik-cache/x86_64/boot.oat (art_jni_trampoline+167)
```

ART의 JNI 트램펄린이 ARM64 라이브러리(`libswappywrapper.so`) 주소를 호스트 코드로 실행했다. 네이티브 메서드를 등록할 때 게스트 함수 포인터가
트램펄린으로 감싸지지 않은 것으로 보인다. A15 ART는 클래스 로더 네임스페이스가 네이티브 브리지일 때만 `GenerateNativeBridgeTrampoline()`을 부르고,
Digitalis는 `GuestMapShadow::IsExecutable()`이 참인 주소만 감싼다. 둘 중 어디서 빠졌는지는 아직 모른다(`jni-next-crash-source.txt`).
ISO `OME-api35-20261005-jni1-digitalis.iso` SHA-256 `9bb72cc07bba1fa1609faf020a4b1814a39ee6412370c972d41b10ea9a2899ee`.

## Swappy 충돌의 원인 (2026-10-06, jni2~jni4)

- jni1 ISO가 0.9 GB 작았던 것은 `installclean` 뒤 남은 커널 산출물 때문에 펌웨어 복사(`copy-firmware.sh`)가 건너뛰어진 결함이었다
  (`system/vendor/firmware/` 4,301개). `build-image.sh`의 `reset_kernel_firmware()`를 고쳤고 jni3 ISO에서 4,301개가 모두 있다(`jni3-firmware-verification.txt`).
- 재현기의 `JNI_OnLoad`에서의 `RegisterNatives`와 `dlopen`한 두 번째 라이브러리 함수의 `RegisterNatives`는 둘 다 통과.
- 기본 꺼짐 계측(`debug.ome.jni_trace=1`)으로 본 게임: `SwappyDisplayManager.nSetSupportedRefreshPeriods`와 `nOnRefreshPeriodChanged`를 등록할 때
  ART가 `namespace_bridged=0`으로 판정해 ARM64 주소를 그대로 등록했고, 충돌한 호스트 PC가 그 주소와 같다(`jni3-game-trace-excerpt.txt`).
  라이브러리를 올린 클래스 로더와 메서드의 클래스 로더가 달라 생기는 경우다.
- ART main은 이 경우 `NativeBridgeIsNativeBridgeFunctionPointer(fnPtr)`로 한 번 더 확인한다(b/393035780). A15 `libnativebridge`에는 이 API와
  콜백 필드가 없어 조건만 옮기면 컴파일되지 않는다(`jni4-*.txt`). Digitalis 본체는 이 콜백(`isNativeBridgeFunctionPointer`)을 이미 구현한다.

## 네이티브 브리지 v8 역이식 뒤 (2026-10-06, jni6)

상류 ART `3428f9be71b0d20978d11d63b5d82962b88d02bf`(`isNativeBridgeFunctionPointer`, 인터페이스 8)와 `6fbb37a7513bf93f63f7a84925cc4d9913590abd`
(RegisterNatives에서 함수 주소로 한 번 더 판정)를 A15에 옮겼다(`guest/build/patches/digitalis/art/0001-*.patch`, 실험 스위치 안).
브리지 버전이 8보다 낮으면 새 콜백 필드를 읽지 않는다. 재현기의 '다른 클래스 로더' 경우는 jni3에서 SIGSEGV, jni6에서 통과(`jni5-cross-before-*`, `jni6-cross-after-*`).

게임: 두 SIGSEGV 모두 사라졌고 `SwappyWrapperInit() succeeded`, 전체 화면 안내와 알림 권한 화면까지 갔다. 알림을 거부한 뒤 흰 화면에 머물렀고
리소스 내려받기 안내가 나오지 않았다. 프로세스는 살아 있다. logcat에 `Firebase modules failed to initialize: messaging (missing dependency)`,
Crashlytics의 `TypeInitializationException`, `com.google.android.gms` 없음이 있다. 이 이미지에는 구글 앱이 없다. 흰 화면의 원인이 GMS 부재인지
번역기 결함인지는 아직 가르지 못했다(`game-jni6-*`).

## 대조 실험: 흰 화면은 GMS 부재 때문 (2026-10-06)

공식 Bliss 16.9.7 GApps ISO(SHA-256 `17137711…e3751`, 매니페스트와 일치)를 제품 도우미의 레거시 길로 scratch 디스크에 설치하고 같은 APK로 시험했다.

| 조건 | 화면 | Firebase messaging |
|---|---|---|
| A13 공식, libndk, GMS 켬 | 권한 → 인트로 → 타이틀과 서버의 업데이트 필수 알림 | 오류 없음 |
| 같은 디스크, 게임 데이터만 지우고 `pm disable-user com.google.android.gms` | 권한 뒤 흰 화면 | `missing dependency`, Crashlytics `TypeInitializationException` |
| A15 자체 이미지, Digitalis jni6, GMS 없음 | 권한 뒤 흰 화면 | 같은 오류 |

GMS를 쓸 수 없으면 트릭컬은 초기화를 마치지 못한다. 흰 화면은 Digitalis의 결함이 아니다. 같은 API 35에서 GMS가 있을 때 그 뒤가 도는지는 아직 확인하지 않았다.
서버는 10644에 업데이트를 요구한다("새로운 버전이 확인되었습니다. 안정적인 서비스 이용을 위하여 스토어로 이동하여 업데이트 해주세요.", `game-gms-on-prompt.png`).
다음 게임 시험에는 최신 APK가 필요하다.

## 사용자 PC 조립 시제품: GApps를 넣은 A15 + Digitalis (2026-10-06)

`dl.google.com`의 `google_apis_playstore/x86_64-35_r09.zip`(색인 SHA-1 `2f0054868e6aab3c098acd3decba17a82aed4176`, 1,762,061,559바이트,
`uses-license` `android-sdk-license`, 계산한 SHA-256 `1fb5e5fd…6dfe6`)에서 `guest/build/gapps/assemble.py`(개발 전용)가 GPT와 LP 메타데이터를 읽어
product와 system_ext를 열고 GmsCore, Phonesky, GoogleServicesFramework와 그 패키지 항목만 남긴 권한·sysconfig XML 넷을 꺼내 debugfs로
jni6 system.img에 넣었다(라벨 `system_file`, 다시 읽어 해시와 라벨 확인, `e2fsck -fn` 통과). 번역기, Widevine, 기기 속성, SELinux 정책은 넣지 않았다.
꺼낸 파일과 조립한 이미지는 이 PC의 `%LOCALAPPDATA%\OpenMobileEmulator\p2\gapps\`에만 있다(R3).

- 부팅 뒤 GMS `24.23.35`가 x86_64로 돌고(`gms`, `.persistent`, `.unstable`), 네이티브 브리지는 그대로 `libberberis_arm64.so`.
- 게임: Firebase messaging 의존성 오류가 사라졌다. 권한 화면 뒤 인트로 단계에서 검은 화면으로 머물렀고(프로세스 살아 있음, 치명 오류 없음)
  업데이트 필수 알림에는 가지 못했다. logcat에 `AndroidVideoMedia`, `Cannot Prepare a disabled VideoPlayer`. A13 기준선에서는 인트로가 검은 채로 나와도
  탭 한 번에 넘어갔다.

## 인트로 검은 화면 진단 (2026-10-06)

- 인트로 영상은 logcat 기준 AVC 1600x1024 + AAC다(APK 자산 목록의 영상은 `assets/meta.mp4` 하나, 346,513바이트. 이 파일인지는 확인하지 않았다). A15에서는 `c2.android.avc.decoder` 생성이 성공했고
  SurfaceFlinger의 게임 레이어는 초당 약 39~42 프레임을 내지만 내용이 검다. 창이 멈춘 것은 아니다(`video-sf.txt`, `video-latency-*.txt`).
- A13 기준선에서는 같은 디코더가 곧바로 죽었고(`Codec2 component "c2.android.avc.decoder" died`, `NdkMediaCodec ... -38`) 유니티가 영상을 건너뛰어 탭 한 번에 타이틀로 갔다
  (`video-a13-codec-comparison.txt`).
- minigbm 로그에 `virgl_add_combination: Skipping unsupported combination format:842094158`(NV12)이 있다. 디코더 출력 형식(YUV)을 virgl 그래픽 버퍼로 만들 수 없는 것이
  두 이미지 공통의 원인일 가능성이 있다(확인 중). UnityMain은 게스트 JIT 안의 futex/poll에 있고 치명 오류는 없다(`video-stack-root.txt`, `video-thread-waits.txt`).
- 정리: `p2\gapps\`의 중간 이미지 세 개를 지워 C: 여유가 22.6 GB에서 34.7 GB가 되었다(`video-disk-cleanup.txt`).
