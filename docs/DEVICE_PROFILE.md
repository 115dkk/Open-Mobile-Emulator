# 기기 호환성 프로필

2026-10-08 사용자 결정: 기기 인증 관련 호환성 처리를 일률적으로 금지하던 D8·R7·R8과
ADR-0012의 해당 조건을 철회한다. 에뮬레이터의 모든 구성과 동작이 완전히 깨끗하다는 전제를
요구하기보다, 호환성 구현과 한계를 공개하는 오픈소스 원칙을 유지한다.

## Galaxy S26 Ultra 후보

`manifests/device-profile.prop`의 출처는 [SM-S948B 소유자의 공개 진단 로그](https://github.com/appsfolder/livebridge/issues/112)다.
2026-06-08에 작성된 자료를 2026-10-08에 확인했다. Samsung 펌웨어에서 직접 추출하거나
현재 Google 인증 여부를 독립적으로 확인한 자료는 아니다.

| 필드 | 값 |
|---|---|
| 모델 | SM-S948B |
| 제조사·브랜드 | samsung |
| 제품 / 코드명 | m3qxxx / m3q |
| 지문 | `samsung/m3qxxx/m3q:16/BP4A.251205.006/S948BXXS2AZE3_OWO2AZE3:user/release-keys` |

지문의 Android 16과 release-keys는 원본 기기의 정보다. OME의 실행 중인 OS 버전, 서명 키,
CPU 명령어 집합 또는 보안 상태를 뜻하지 않는다. 실제 SDK·ABI·보안 패치 수준·ADB 인증 설정은
게스트 값을 유지한다. 빌드 출처 증명(ADR-0007)과 Google 기기 인증은 별개다.

## 적용 경로

- 새 자체 이미지: `build-image.sh`가 `device_profile.py`로 여섯 속성의 이름·중복·형식·상호
  일치 여부를 검사하고 `vendor/extra`를 통해 `/system/etc/ome-device-profile.prop`을 넣는다.
  init의 `0002-load-ome-device-profile.patch`가 vendor 초기화 뒤에 공개 모델명과 빌드 지문을
  적용한다. 제품 코드명과 파티션별 식별값은 유지한다. SDK·ABI·검증 부팅 상태는 이 로더의 허용 목록에 없다.
- 기존 Android-x86 게스트: `guest/build/apply_device_profile.py`는 명시한 localhost adb 대상으로만
  작동한다. root와 x86_64 ABI를 확인하고 모든 속성 파일을 호스트에 백업한 다음, 기존 파일의
  공개 모델명과 빌드 지문만 변경한다. 쓰기 후 내용을 대조하고 원래 권한을 복원한다. 중간 실패 때는 이미
  변경한 파일을 백업으로 되돌린다. 부팅 및 Google 호환성까지 보장하는 트랜잭션은 아니다.
- 이전 Bliss 훅을 끄는 패치는 유지한다. 선택한 S26 프로필을 Pixel 등 다른 값으로 덮어쓰거나
  그래픽·기능 탐지에 부작용을 만드는 것을 막기 위한 선택이다. 실기기 프로필 금지 규칙은 철회했다.

```powershell
py -3 guest/build/device_profile.py manifests/device-profile.prop
py -3 guest/build/test_device_profile.py
# 실행 중인 게스트를 adb root로 연결한 뒤, 새 백업 경로를 지정한다.
py -3 guest/build/apply_device_profile.py --adb <adb.exe> --serial 127.0.0.1:5555 --profile manifests/device-profile.prop --backup-dir <새-백업-폴더>
```

현재 마이그레이션 도구는 ext4 system-as-root의 Android 13 개발 게스트용이다. 읽기 전용 EROFS,
root가 없는 이미지 또는 다른 파티션 구성에는 적용하지 않는다. WHPX에서는 게스트 자체 reboot가
멈출 수 있으므로 정상 종료 후 QEMU를 다시 시작한다. 계정이나 Play Store 데이터를 지우는 과정은 없다.

## 검증 기준

`getprop`의 모델·지문 일치, 재부팅 성공, Google 로그인, Play Protect 인증 표시,
앱 상세 페이지의 설치 가능 여부, 실제 내려받기/업데이트, 결제, Play Integrity 판정을 각각 기록한다.
[Google 안내](https://support.google.com/googleplay/answer/7165974?hl=en)의 기기 인증과
[Play Integrity 판정](https://developer.android.com/google/play/integrity/verdicts)은
모델명이나 지문 문자열이 맞는지만 확인하는 절차가 아니다. 지문 하나로 정상 사용을 보장할 수 없다.

구현 검증: 프로필 테스트 3개, SPDX 검사, 산출물 매니페스트 검사, 빌드 셸 문법 검사 통과.
새 init 패치는 고정된 WSL Bliss 소스에 `git apply --check`를 통과했다. 새 Android 15 이미지 전체
컴파일·부팅은 아직 수행하지 않았다.

Android 13 기존 게스트에서는 모델명·지문 적용과 재부팅을 확인했다. 실제 스토어 차단을 해소한 변경은
GLES 기능 보고를 3.0에서 실제 지원하는 3.2로 바로잡은 것과 스토어 캐시 갱신이었다.
트릭컬을 `10644`에서 `10655`로 Play 스토어를 통해 업데이트한
[시험 기록](evidence/device-profile/20261008/play-store.md)을 참조한다.
