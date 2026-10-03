# ADR-0012: 자체 게스트 이미지는 개발 PC에서 빌드해 사전 릴리스로 내고, 안드로이드 15 게스트의 adb는 설치 때 넣은 키로 승인한다

상태: 채택, 2026-10-02(사용자 결정). ADR-0007의 "개발 PC에서 만든 파일은 릴리스에 올리지 않는다"에 예외를 두고,
ADR-0006과 R3의 배포 방식을 자체 이미지에 적용하며, ADR-0010의 도우미 설치를 Bliss 18 트리에 맞춘다. 근거 사실은
`docs/evidence/P3/build-20261002.md`와 `docs/P3-PLAN.md`에 있다.

## 배경

P3의 첫 자체 빌드가 2026-10-02 새벽에 끝났다. 입력은 `guest/build/pins.env`와 `guest/build/manifest/ome.xml`(1504 프로젝트의
커밋 스냅샷)이고, 산출물은 블롭 없는 안드로이드 15 ISO(Bliss 18.4 `voyager-x86`, vanilla, 2,946,441,216 B,
SHA-256 `62dc0836…e683`)다. 시스템 이미지에 R1의 금지 패턴에 걸리는 파일은 없고 네이티브 브리지는 꺼져 있다. 제품의 QEMU로
라이브 부팅하면 안드로이드 15가 잠금 화면까지 올라오고 virgl로 그린다.

그 이미지를 제품에 넣으려면 둘을 정해야 했다.

첫째, 어디서 빌드해 어떻게 배포하느냐. ADR-0007은 릴리스 바이너리를 러너에서만 빌드하고 출처 증명을 붙이기로 했다. 그런데
안드로이드 트리 빌드는 디스크 250 GB와 메모리 48 GB(이번 빌드의 실측), 4~6시간이 들어 GitHub 호스트 러너(디스크 14 GB,
메모리 16 GB)에서는 돌지 않고, 대형 러너는 개인 계정 저장소에서 쓸 수 없다(`docs/P3-PLAN.md` 5.1절). 남는 길은 개발 PC
빌드이거나 유료 VM이었다.

둘째, 안드로이드 15 게스트의 adb를 어떻게 승인하느냐. Bliss 18의 userdebug 빌드는 `ro.adb.secure=1`이라 adbd가 호스트
키의 승인을 요구한다(`packages/modules/adb/daemon/main.cpp`: debuggable 빌드에서는 `ro.adb.secure` 값이 곧
`auth_required`). 안드로이드 13 프로필(Bliss 16.9.7)은 `ro.adb.secure=0`이라 이 문제가 없었다. 라이브 부팅에서는 루트
콘솔로 호스트의 `adbkey.pub`을 `/data/misc/adb/adb_keys`에 넣자 바로 `device`가 되었다. `androidboot.insecure_adb=1`은
`ro.boot.insecure_adb`로만 들어가고 `ro.adb.secure`를 바꾸지 못했다(adbd는 그 속성을 읽지 않는다).

## 결정

1. **자체 게스트 이미지는 개발 PC의 WSL2(`OME-Build`)에서 빌드해 GitHub Releases에 사전 릴리스로 올린다**(사용자 결정
   2026-10-02). 이것은 ADR-0007의 예외다. 그 ADR이 러너 빌드를 요구한 이유(바이너리가 공개된 소스와 워크플로에서 나왔다는
   증명)는 그대로 유효하므로, 출처 증명을 붙이지 못하는 대신 다음을 같은 릴리스에 올린다. 빌드 입력 전부(`pins.env`,
   `ome.xml` 스냅샷), 산출물의 `build.prop`, 빌드 로그, 금지 패턴 검사 결과, 기계 생성한 `NOTICE-guest.md`, 그리고 빌드와
   확인 과정의 증거 문서 경로. 릴리스 이름은 `guest-android-15-<날짜>`이고 사전 릴리스 표시를 유지한다. 출처 증명을 붙일 수
   있는 빌드 호스트(짧은 수명의 자체 호스팅 러너 등)가 생기면 그때 이 예외를 거둔다.
2. **배포 단위는 ISO 하나이되 GitHub 자산 한도 때문에 조각으로 올린다.** GitHub Releases는 파일 하나를 2 GiB 아래로 제한하므로
   2.95 GB ISO는 1 GiB(1,073,741,824 B) 조각 `…iso.part0`, `.part1`, `.part2`로 나눈다. `manifests/artifacts.json`의 항목은
   전체 파일의 `filename`, `size_bytes`, `sha256`에 더해 `parts` 배열(조각마다 `filename`, `url`, `size_bytes`, `sha256`)을
   갖고, `url`은 릴리스 페이지를 가리킨다. 제품의 산출물 저장소(`ome-artifacts`)는 조각을 차례로 받아 한 파일로 이어 붙이고
   조각마다 SHA-256을 대조한 뒤 전체 SHA-256을 다시 대조한다. 이어받기는 지금처럼 파일 길이로 한다. 압축(zstd 등)은 제품에
   새 의존성을 부르고 EROFS 시스템 이미지는 이미 압축되어 있어 고르지 않았다.
3. **이미지 프로필 `ome-android-15`**(`manifests/images/ome-android-15.json`)는 `distribution: self_built`, `translator: none`,
   `install: helper_boot`, `adb_secure: true`, `status: candidate`로 들어간다. 프로필 필드 `adb_secure`는 그 이미지의 adbd가
   키 승인을 요구한다는 뜻이고, 제품은 이 값이 참인 프로필을 설치할 때 호스트 adb의 공개키를 게스트에 넣는다(4번).
   마법사는 `candidate` 프로필을 '검증 전' 표시와 함께 고르게 둔다. 능력 조사를 통과하면 `verified`로 바꾼다.
   정정 2026-10-03(사용자 결정): 능력 조사 통과만으로는 바꾸지 않는다. 마법사는 `verified` 가운데 최신을 기본으로 고르므로
   (`recommended_index`), 변환기가 없는 이 프로필이 `verified`가 되면 새 사용자의 기본 이미지에서 arm64 게임이 돌지 않는다.
   완료 실행 a15-6과 a15-7이 내려받기부터 능력 조사까지 통과했지만, 8절 9번의 변환기 흐름이 정해져 게임까지 돌 때 바꾼다.
4. **adb는 설치 때 넣은 키로 승인한다**(사용자 결정 2026-10-02). 제품은 설치를 시작하기 전에 `adb start-server`로 호스트
   adb 클라이언트의 키 쌍이 있게 하고(`client/auth.cpp`의 `load_userkey`가 없으면 만든다) `%USERPROFILE%\.android\adbkey.pub`을
   읽어(`adb_utils.cpp`의 `adb_get_android_dir_path`는 윈도우에서 `CSIDL_PROFILE` 아래 `.android`를 쓰고 환경 변수로
   바꾸는 길이 없다) 도우미 initrd에 `/ome/adb_keys`로 넣는다. 도우미는 `system.img`를 디스크에 복사한 뒤 그 이미지 루트에
   `/adb_keys`를 쓴다. 이 자리는 adbd가 `/data/misc/adb/adb_keys`보다 먼저 보는 키 파일이다
   (`frameworks/native/libs/adbd_auth/adbd_auth.cpp`의 `key_paths`). 정정 2026-10-03: 빌드된 이미지에서 `/adb_keys`는
   없는 파일 `/product/etc/security/adb_keys`를 가리키는 심볼릭 링크라 그냥 복사하면 실패했고(완료 실행 a15-4의 `keys-copy`),
   시스템 이미지는 실행 중에 읽기 전용으로 붙으므로 `early-init`의 `restorecon /adb_keys`가 라벨을 고쳐 준다고 기대할 수 없다.
   그래서 도우미는 이미지를 마운트하지 않고 `install.sfs`의 `debugfs`로 링크를 지우고 일반 파일을 쓴 뒤 SELinux 라벨을
   `plat_file_contexts`의 `/adb_keys` 항목과 같은 `u:object_r:system_file:s0`로 직접 넣고, 다시 읽어 종류와 라벨과 내용을 확인한다.
   정정 2026-10-03: 키만으로는 부족했다. 이 이미지는 lunch가 `userdebug`인데도 `ro.debuggable=0`이고
   `persist.sys.usb.config`이 없어, 첫 부팅이 `none`을 저장하고 adbd를 띄우지 않는다(a15-5). 그래서 같은 도우미가
   `/system/build.prop`에 `persist.sys.usb.config=adb`를 덧붙인다(원래 모드와 라벨 유지, 이미 있으면 건너뜀). `init.usb.rc`가 부팅 때
   이 값을 `sys.usb.config`로 옮기고 `AdbService`가 이 값으로 `adb_enabled`를 정한다. 다음 이미지부터는 `guest/build/build-image.sh`가
   Bliss의 덮어쓰기 자리 `vendor/extra/product.mk`에 같은 속성을 넣는다. adb 포트는 호스트의 127.0.0.1에만 열리므로 이 변경으로
   게스트 adb가 다른 PC에 열리지는 않는다(M2 6-2의 다른 PC 연결은 여전히 기본 끔).
   `/data/misc/adb/adb_keys`에 쓰는 길은 고르지 않았다. `init.rc`는 `/data`와 `/data/misc`만 `restorecon`하고 그 아래를 재귀로
   라벨하지 않으므로 initrd가 만든 파일은 `unlabeled`로 남는다. 키를 읽지 못하는 상태(사용자가 `.android`를 지운 경우)는
   게스트를 다시 설치해 푼다. 안드로이드 13 프로필은 `adb_secure`가 거짓이라 아무것도 넣지 않고 지금 흐름 그대로다.
5. **도우미 설치는 Bliss 18의 initrd(`bootable/aaropa`)에 맞춘 두 번째 길을 갖는다.** 같은 스크립트 `99-ome-install`이
   환경을 보고 고른다. `/src/install.img`가 있으면 Bliss 16의 길(지금 그대로), `/mnt/install.sfs`와 `/mnt/system.efs`가
   있으면 aaropa의 길이다. aaropa의 `/init`은 `/scripts/*`를 시스템 이미지를 마운트하기 **전에** source하고(346행), 시스템
   이미지의 안드로이드 15 바이너리는 APEX가 평탄화되지 않아 initrd에서 돌릴 수 없다(`/system/bin/linker64`가
   `/apex/com.android.runtime/…`을 가리킨다). 그래서 aaropa의 길은 `install.sfs`(설치기 환경, 데비안 계열 squashfs)를 루프
   마운트해 `chroot`로 그 안의 `sgdisk`와 `mke2fs`를 쓰고, `system.efs`를 EROFS로 마운트해 `system.img`를 꺼내며, 복사
   진행률은 `pv` 대신 목적 파일 크기를 주기적으로 읽어 보고한다. 디스크 배치(`/ome/{kernel,initrd.img,system.img,data/}`)와
   직접 커널 부팅(`-append "root=/dev/ram0 SRC=/ome …"`)은 ADR-0010과 같다. aaropa initrd의 `build-fstab`가 그 폴더에서
   `system.img`, `kernel`, `initrd.img`, `data/`를 찾아 fstab을 만들므로 다른 표식은 필요 없다.
6. **소스 제공.** 이미지에는 GPL 구성요소(커널, busybox, e2fsprogs, alsa-utils 등)가 든다. 릴리스에는 커널 소스 묶음
   (`kernel/x86/common`의 고정 커밋을 `git archive`한 tar.gz와 `.config`)을 올리고, 나머지 구성요소의 소스는 `ome.xml`
   스냅샷이 가리키는 공개 저장소의 커밋이며 `NOTICE-guest.md`가 프로젝트마다 이름, 경로, 저장소 URL, 커밋, 라이선스 표식을
   적는다. 트리 전체(약 100 GB)를 묶는 것은 사전 릴리스 단계에서는 하지 않는다. 이것은 알려진 한계로 적는다.

## 결과와 영향

- CLAUDE.md R3의 "CI에서 이 변형의 빌드 성공을 게이트로 둔다"는 개발 PC 빌드 기록(`docs/evidence/P3/build-*.md`)으로 대신한다.
  CLAUDE.md P3 항목과 ADR-0007의 결과 절에 이 예외를 적는다.
- `docs/NETWORK.md`의 GitHub Releases 행에 게스트 이미지 내려받기를 더한다. 호스트는 이미 목록에 있다
  (`github.com`에서 `objects.githubusercontent.com`으로 넘어간다).
- `ome-artifacts`에 조각 내려받기, `ome-guest-image`에 `adb_secure`, `ome-adb`에 호스트 공개키 읽기, `ome-guest-install`에
  키 파일을 담은 initrd 만들기와 aaropa 길의 도우미, `ome-runtime`에 설치 전 키 준비가 생긴다. 키를 준비하지 못하면 설치를
  시작하지 않고 `adb_host_key_unavailable`로 알린다. `ci/Check-Manifest.ps1`은 `parts`를 검사한다.
- `guest/build/`에 릴리스 자산을 만드는 스크립트(`make-release-assets.sh`, `make-notice-guest.sh`, `make-kernel-source.sh`)가
  생긴다. 릴리스 자체는 `gh release create --prerelease`로 개발 PC에서 올린다. 워크플로 `guest-image-release.yml`은 러너
  빌드가 가능해질 때 만든다.
- 안드로이드 15 게스트의 `adb root`는 Bliss의 `adbroot_service`가 막는다. 루트 토글(M2 6-2)은 이 ADR의 범위 밖이고
  `docs/P3-PLAN.md`에 남긴다.

## 열린 문제

- 호스트 adb 키가 바뀌면(사용자가 `.android`를 지우거나 다른 PC로 홈을 옮기면) 게스트가 `unauthorized`가 된다. 제품은 그
  상태를 adb 장치 목록에서 알 수 있으므로, 다시 설치 대신 키를 다시 넣는 길(루트가 있을 때 `/data/misc/adb/adb_keys`에 쓰기)은
  루트 토글과 함께 본다.
- 릴리스 이미지의 서명은 test-keys다. 릴리스 서명 키를 둘지는 정하지 않았다.
