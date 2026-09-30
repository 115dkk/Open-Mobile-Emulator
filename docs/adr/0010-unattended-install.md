# ADR-0010: 운영체제 설치는 사용자의 승인 뒤 제품이 숨은 도우미 부팅으로 무인 수행하고, 게스트는 GRUB 없이 직접 커널 부팅한다

상태: 채택, 2026-10-01(사용자 승인). 승인하면서 하나를 고쳤다. 승인 단추는 `디스크 만들기`가 아니라 **`설치하기`**이고,
화면 설명은 다른 설치 마법사들의 관례대로 "여유 공간이 충분해 설치할 수 있습니다"는 식으로 둔다. 그래서 화면
수정이 하나 생긴다(S1.5의 단추 이름과 설명 문장). M2 기능 명세 1번과 9번(개정 2026-10-01)의 구현 방법을 정하는
ADR이며, 게스트 디스크의 배치와 부팅 방식을 바꾼다. 구현 전에는 지금의 대화형 설치 화면(개발용)이 그대로 남는다.

## 배경

원래 요구는 사용자가 설치를 **승인**하는 것이었는데 M2 명세가 이를 설치기 화면을 직접 조작하는 뜻으로 오독했다
(CLAUDE.md 개정 2026-10-01). 지금 S1.5는 무대 안에 Bliss 설치기를 띄우고 안내 여섯 줄을 보여 사용자에게 파티션과
포맷과 GRUB 설치를 시킨다. 완료 실행 드라이버는 OCR과 키 입력으로 그 설치기를 넘기지만(`ci/dod/run-dod.mjs`,
개발 PC 28회차까지) 그것은 시험 장치이고, 제품 사용자에게는 GRUB과 cfdisk가 그대로 노출된다. 불안한 사용자가
화면을 조작해 실패를 만들 것이므로 설치는 사용자 입력 없이 진행 화면만으로 끝나야 한다(M2 완료 기준).

설치기 화면을 가리고 제품이 직접 깐다면 그 화면을 조작할 이유도 없다. 그래서 설치기를 자동으로 넘기는 대신,
제품이 쓸 수 있는 더 효과적인 설치 방법을 찾았다.

## 조사 결과 (2026-10-01, ASTRA 워커 둘, 소스 열람만 하고 실행은 하지 않음)

### 설치기의 무인 모드는 무인이 아니다

Bliss 16의 설치기(`BlissRoms-x86/bootable_newinstaller`, 브랜치 `arcadia-x86`, 커밋 4b83b9ac, 2024-09-04. ISO의
매니페스트 a3645f97이 이 브랜치를 가리키지만 ISO 안의 initrd와 대조한 것은 아니다)에는 `AUTO_INSTALL` 커널
인자가 있지만 다음 때문에 키 없이 끝나지 않는다.

- `initrd/init` 114~122행: `AUTO_INSTALL`이 있으면 "Are you sure you want to continue (y/n)?"를 콘솔에서 `read`로
  묻는다. 판정이 뒤집혀 있어 Y로 답하면 라이브 모드로 간다. 2024-03-03 SourceForge 스레드의 "Auto Install이
  라이브로 부팅한다"는 보고와 맞아떨어진다.
- `install/scripts/1-install`: 자동 응답(`set_answer_if_auto`)이 닿지 않는 대화상자가 남는다. 파일 시스템 레이블
  입력(298~300행), ESP 포맷과 선택(1004~1036행), OTA 준비 질문(854~857행), 마지막 실행/재부팅 메뉴(1157~1160행).
  `AUTO_INSTALL=force`는 지우기 확인 하나만 건너뛴다. 완료 뒤 자동 종료는 없다.
- 상류 android-x86(pie)은 y/n 읽기가 없고 GRUB2 EFI 질문을 자동으로 받지만, 마지막 실행/재부팅 메뉴는 그대로다.
- Bliss의 다른 계열(`Bliss-Bass/platform_bootable_newinstaller`, 2023-12 WIP)에는 `OEM_INSTALL=force|mbr|efi|update`가
  있으나 16.9.7이 쓰는 계열이 아니다.

출처: https://github.com/BlissRoms-x86/bootable_newinstaller/blob/4b83b9ac9e844b73f858d725c8bf65e139fbd62a/install/scripts/1-install ,
https://github.com/BlissRoms-x86/bootable_newinstaller/blob/4b83b9ac9e844b73f858d725c8bf65e139fbd62a/initrd/init ,
https://sourceforge.net/p/blissos-x86/discussion/general/thread/5f58a2ff61/

### initrd가 하는 일과 끼어들 자리

- 커널은 점 없는 `이름=값` 인자를 init의 환경 변수로 넘긴다. Bliss initrd는 `ROOT=`(찾을 장치), `SRC=`(시스템
  파일이 있는 디렉터리), `DATA=`(데이터 장치나 디렉터리)를 읽고, 장치를 훑어 `system.sfs`나 `system.img`나 풀린
  `system/`이 있는 파일 시스템을 `/mnt`에 마운트한다. 그 뒤 `ls /scripts/* /src/scripts/*`의 파일을 모두 셸 조각으로
  `source`한다. `INSTALL=1`이면 그 전에 `/src/install.img`를 `/`에 cpio로 풀어 설치기와 도구(sgdisk, mkfs.ext4,
  grub 도구, EFI 파일)를 얻고 `do_install`을 부른다.
- 리눅스는 initramfs로 cpio 아카이브 여러 개를 이어 붙인 것을 받는다(압축과 비압축 혼용 가능). 뒤에 오는 같은
  경로의 파일이 앞의 것을 덮는다. 그러므로 ISO의 `initrd.img` 뒤에 우리 cpio를 붙여 `/scripts/99-ome-install`을 넣을
  수 있다. 다만 `INSTALL=1`을 함께 주면 install.img가 `/scripts/1-install`을 다시 덮으므로, `INSTALL`과
  `AUTO_INSTALL`과 `DEBUG`는 주지 않고 우리 스크립트 이름을 따로 두어야 한다.
- initrd는 BusyBox의 `fdisk`와 `mke2fs` 링크를 지운다. 포맷 도구는 install.img 안에 있으므로 우리 스크립트가
  install.img를 직접 풀어(또는 필요한 도구만 꺼내) 써야 한다. 정확한 도구 목록과 동적 라이브러리는 ISO 안을 열어
  확인해야 한다.
- 데이터의 마운트 순서는 `DATA=` 장치, `SRC/data` 디렉터리, `SRC/data.img`(loop), 그 밖은 tmpfs다. FAT나 NTFS
  위에서는 ext4 `data.img`(loop)를 써야 하고, FAT32의 4 GiB 파일 한도 때문에 게임 데이터에는 맞지 않는다.
- Bliss 16(안드로이드 13)은 `ramdisk.img`가 시스템에 합쳐져 있어 따로 필요 없다.
- 설치기가 EFI에 쓰는 것은 shim(`BOOTx64.EFI`)과 `grub-mkimage`로 만든 `grubx64.efi`와 `/EFI/BlissOS/grub.cfg`다.
  `BOOTx64.EFI`만 복사해서는 GRUB이 뜬다고 볼 수 없다.

출처: https://github.com/BlissOS/bootable_newinstaller/blob/bec189cd65d7a789ee3f0242e9df7f4296368ee7/initrd/init ,
https://raw.githubusercontent.com/BlissOS/bootable_newinstaller/typhoon-x86/initrd/scripts/2-mount ,
https://docs.kernel.org/driver-api/early-userspace/buffer-format.html ,
https://docs.blissos.org/installation/manual-install-on-linux/

### 선례

- Xtr126의 `androidx86-installer-qemu-linux`는 설치기와 GRUB을 둘 다 건너뛴다. 원시 이미지에 `mkfs.ext4`를 하고
  마운트해 시스템 파일을 복사한 뒤, 매번 QEMU가 ISO의 커널과 initrd를 직접 부팅한다
  (`-kernel ... -initrd ... -append "root=/dev/ram0 quiet SRC=/ ..."`). 리눅스 호스트 전용이지만 initrd의 `SRC=`
  탐색만으로 설치본이 도는 것을 보여 준다. https://github.com/Xtr126/androidx86-installer-qemu-linux
- ExtremeGTX의 Android-x86 Installer for Windows는 7-Zip으로 ISO에서 파일을 꺼내고 Cygwin `mke2fs.exe`로
  `data.img`를 만들어 기존 FAT32/NTFS 파티션의 폴더에 설치한다. 윈도우에서 만든 선례이지만 ext4 도구를 따로
  들고 다닌다. https://github.com/ExtremeGTX/Androidx86-Installer-for-Windows
- Genymotion, 안드로이드 스튜디오 에뮬레이터, Waydroid는 모두 미리 만든 이미지를 복사할 뿐 설치기를 돌리지
  않는다. 그러나 이 제품은 R2와 R3 때문에 이미지를 재배포할 수 없으므로 사용자 PC에서 만들어야 한다.

### 호스트에서 직접 만드는 길(A)의 조건

GPT와 FAT32는 Rust 크레이트(`gpt`, `fatfs`)로 쓸 수 있고 원시 이미지를 `qemu-img convert`로 qcow2로 바꾸면 된다.
ext4는 윈도우에서 쓰는 도구가 있기는 하다(lwext4와 그 Rust 래퍼 `ext4-lwext4`, `ext4-mkfs`, Cygwin의 e2fsprogs,
Cosmopolitan 빌드의 `unpins/e2fsprogs`). 하지만 어느 것도 이 제품에 맞게 검증되지 않았고, 새 원어 의존성과
라이선스 검토를 부른다. Windows가 포맷하는 VHDX는 FAT32와 NTFS와 exFAT까지만 되고 관리자 권한이 필요하다.
QEMU의 `vvfat`는 쓰기 모드가 베타라 데이터 디스크로 못 쓴다.

## 결정 (제안)

1. 설치는 **도우미 부팅**으로 한다. 사용자가 `설치하기`로 승인하면 제품이 qcow2를 만들고, 검증된 ISO의
   `kernel`과 `initrd.img`를 호스트로 꺼낸 뒤, `initrd.img` 뒤에 제품이 만든 cpio(`/scripts/99-ome-install`)를 이어
   붙여 QEMU를 **화면 없이** 한 번 띄운다(`-display none`, ISO는 읽기 전용 CD-ROM, 쓰기 가능한 장치는 새 qcow2 하나,
   `-no-reboot`). 커널 인자에는 `INSTALL`, `AUTO_INSTALL`, `DEBUG`를 주지 않는다. 스크립트는 install.img에서 도구를
   꺼내 디스크를 GPT로 나누고(ESP 없이 ext4 하나로 충분한지는 2번에 따른다) `system.img`(system.sfs 안의 쓰기 가능한
   이미지, 6절 번역기 계약이 system 쓰기를 요구한다)와 `kernel`과 `initrd.img`를 복사하고 `data/`를 만들고, 진행
   상황을 QEMU의 시리얼(`-serial file:`)이나 가상 디스크의 표식 파일로 알린 뒤 `poweroff -f`한다. 제품은 QEMU
   종료 코드와 표식으로 성공을 판정하고 진행 화면만 보인다.
2. 설치된 게스트는 **GRUB 없이 직접 커널 부팅**한다. 제품이 이미 QEMU 명령줄을 쥐고 있으므로 매 시작에
   `-kernel <홈의 kernel> -initrd <홈의 initrd.img> -append "root=/dev/ram0 SRC=/blissos <표시 인자> quiet"`를 준다.
   그러면 ESP도 GRUB도 efibootmgr도 필요 없고, 표시 설정(M2 5번의 `video=`, 주사율)은 GRUB 환경 대신 `-append`로
   바로 간다. GRUB 화면의 굳음(`embedded-display-freeze.md`)이 생길 자리도 없어진다. OVMF를 계속 쓸지(QEMU는
   OVMF 아래서도 fw_cfg로 커널을 직접 부팅한다) SeaBIOS로 갈지는 스파이크에서 정한다.
3. 지금의 대화형 설치 화면과 완료 실행 드라이버의 설치기 자동화(OCR과 키, GRUB 선택 막대 판독)는 이 ADR을 채택해
   구현한 뒤 걷어낸다. 드라이버의 S1.5는 진행 화면과 QEMU 종료를 기다리는 단계로 줄어든다.
4. 이미지 프로필(ADR-0004)에는 그 프로필의 설치 방식과 커널 인자를 적는 자리를 둔다. 안드로이드 15 이상의
   자체 이미지(R3)는 처음부터 이 배치로 만든다.

## 결과와 영향

- 사용자가 보는 것은 진행 화면 하나다. 설치 중 화면 조작으로 실패를 만들 길이 없다.
- 제품에 새 원어 의존성이 없다. 포맷과 복사는 ISO가 든 리눅스 도구가 한다(R2, R3 준수. 도구는 사용자 PC에서
  받은 ISO 안에 있다).
- 게스트 디스크 배치가 설치기의 것(ESP + ext4 + shim/GRUB)에서 ext4 하나(`/blissos/{kernel,initrd.img,system.img,data/}`)로
  바뀐다. 기존에 설치기로 만든 게스트(개발용)와 호환되지 않으므로 게스트 기록에 배치 버전을 적고, 옛 배치는
  다시 설치를 안내한다.
- 첫 부팅 인자를 GRUB 항목이 아니라 제품이 정하므로 `guest/kernel-cmdline.md`의 실험 결과가 그대로 `-append`에
  들어간다.

## 검증 절차 (스파이크, 채택 뒤 첫 일)

1. 검증된 16.9.7 ISO에서 `initrd.img`와 `install.img`를 풀어 `/init`, `/scripts/*`, `1-install`, 도구 목록(sgdisk,
   mkfs.ext4, e2fsck, 동적 링커와 라이브러리)을 위 소스와 대조한다. 어긋나면 이 ADR의 사실을 고친다.
2. 개발 PC에서 손으로 도우미 부팅을 한 번 한다. `-display none -serial file:` 로 스크립트 출력만 받고, 끝난 디스크를
   2번 방식으로 부팅해 adb 부팅 표식과 게임 실행까지 간다. 재부팅 뒤 데이터가 남는지, `adb root; adb remount`가
   되는지(6절) 본다.
3. WHPX와 OVMF 조합에서 `-kernel` 직접 부팅이 되는지, SeaBIOS가 더 단순한지 잰다. 부팅 시간을 GRUB 경로와 비교한다.
4. 통과하면 `ome-guest-config`에 도우미 부팅과 직접 부팅의 명령줄을, `ome-runtime`에 설치 단계의 상태 기계(만들기,
   도우미 부팅, 표식 대기, 실패 시 다시 설치)를, 마법사 S1.5에 진행 화면을 만든다. 드라이버는 그 뒤 고친다.

## 열린 문제

- ISO의 실제 initrd가 열람한 소스와 같은지(1번)가 먼저다. 회차에서 본 "Do you want to install EFI GRUB2?" 문구는
  상류의 옛 질문이라 ISO가 `arcadia-x86` 끝 커밋과 다를 수 있다.
- system.img를 쓰기 가능하게 복사하는 대신 system.sfs를 그대로 두고 오버레이로 쓰는 길이 안드로이드 13 Bliss에
  있는지는 보지 않았다. 번역기 교체 계약(6절)이 system 쓰기를 요구하므로 우선은 복사한다.
- 설치 실패의 사용자 안내(디스크 여유, ISO 손상, 도우미 시간 초과)와 다시 설치의 상태 기계는 구현 때 정한다.
