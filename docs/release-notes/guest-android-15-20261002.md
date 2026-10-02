Open Mobile Emulator의 첫 자체 게스트 이미지입니다. 안드로이드 15(API 35), x86_64, BlissOS 18.4 `voyager-x86` 트리를
블롭 없이 빌드한 것이며 사전 릴리스입니다. 제품의 첫 실행 마법사와 설정의 `새 운영체제 설치`에서 `안드로이드 15` 카드를 고르면
제품이 이 릴리스에서 받아 설치합니다. 손으로 받을 일은 없습니다.

## 무엇이 들어 있나

- 네이티브 브리지(ARM 변환기)와 구글 앱이 없습니다. 그래서 ARM 전용 게임은 이 이미지에서 실행되지 않습니다. 지금은 부팅과
  능력 조사, 개발용입니다(`docs/KNOWN_LIMITATIONS.md`).
- 시스템 이미지에 금지 패턴(`libndk_translation*`, `libhoudini*`, `GmsCore*`, `Phonesky*` 등)에 걸리는 파일이 없다는 검사
  결과가 `forbidden-scan.txt`입니다.
- 서명은 AOSP test-keys입니다.

## 파일

| 파일 | 설명 |
|---|---|
| `ome-android-15-x86_64-20261002-db973c9.iso.part0` ~ `.part2` | ISO를 1 GiB 단위로 나눈 조각. GitHub가 자산 하나를 2 GiB 아래로 제한해서입니다. 제품이 차례로 받아 이어 붙입니다. 손으로 합치려면 `copy /b …part0+…part1+…part2 …iso` |
| `ome-android-15-x86_64-20261002-db973c9.iso.sha256` | 이어 붙인 ISO 전체의 SHA-256 |
| `SHA256SUMS` | 모든 자산의 SHA-256 |
| `NOTICE-guest.md`, `NOTICE-guest.xml.gz` | 들어간 구성요소의 이름, 저장소, 커밋, 라이선스 표식(기계 생성)과 이미지 안의 파일별 고지 |
| `ome.xml`, `pins.env`, `build.prop` | 빌드 입력(1504 프로젝트의 커밋 스냅샷, 고정 값)과 산출물의 속성 |
| `kernel-source-*.tar.gz`, `kernel-config-*.txt` | 커널(GPL-2.0) 소스 묶음과 설정 |
| `build-logs.tar.xz`, `forbidden-scan.txt`, `file_list.txt` | 빌드 로그, 금지 패턴 검사, 시스템 이미지 파일 목록 |

## 어디서 어떻게 만들었나

개발 PC의 WSL2(Ubuntu 22.04)에서 `guest/build/build-image.sh`로 빌드했습니다. 안드로이드 트리 빌드는 GitHub 러너에서 돌지
않으므로(디스크 250 GB, 메모리 48 GB) 이 릴리스의 파일에는 빌드 출처 증명이 **없습니다**. 제품 설치기와 QEMU의 릴리스와 다른
점이고, 그 대신 빌드 입력과 로그, 검사 결과를 전부 같이 올렸습니다. 근거와 조건은 `docs/adr/0012-self-built-image-release.md`,
빌드 기록은 `docs/evidence/P3/build-20261002.md`입니다.

## 요구 사항

Open Mobile Emulator 0.1.2 이상(조각 내려받기와 adb 키 주입이 든 판)이 필요합니다. 0.1.1 이하의 제품은 이 프로필을 모릅니다.
