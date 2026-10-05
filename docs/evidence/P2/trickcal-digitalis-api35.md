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
