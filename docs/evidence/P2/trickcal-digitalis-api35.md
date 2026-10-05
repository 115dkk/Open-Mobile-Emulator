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
