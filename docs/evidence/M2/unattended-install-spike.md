# 무인 설치 스파이크 (ADR-0010 검증, 2026-10-01)

ADR-0010이 고른 방법(ISO의 커널과 initrd에 제품 스크립트를 이어 붙인 숨은 도우미 부팅으로 설치하고, 설치된
게스트는 GRUB 없이 직접 커널 부팅)을 개발 PC에서 실기로 확인한 기록이다. 절차는 ADR의 "검증 절차" 1~3번을
따랐고, 결과는 ADR의 "검증 결과" 절에 요약했다. 이 문서는 그 근거다.

## 환경

| 항목 | 값 |
|---|---|
| 호스트 | 개발 PC(CLAUDE.md 9절), Windows 11 22621, WHPX |
| QEMU | 제품 번들 `host/target/debug/qemu`(v11.1.1 c3d48b7 + 패치 0001~0005, exe sha 4558762e) |
| 펌웨어 | 번들의 `edk2-x86_64-code.fd`, 변수 템플릿 `edk2-i386-vars.fd` |
| ISO | `Bliss-v16.9.7-x86_64-OFFICIAL-gapps-20241011.iso`(매니페스트의 SHA-256으로 검증된 제품 홈의 사본) |
| 도구 | 윈도우 내장 bsdtar(`C:\Windows\System32\tar.exe`, libarchive 3.7.7)로 ISO와 cpio를 풀고 묶음. MSYS2 gzip |
| 런처 | `unattended-install-spike/spike.ps1`(이 PC의 절대 경로가 박힌 스파이크용 스크립트) |

## 1. ISO를 풀어 확인한 사실

ADR을 쓸 때는 소스만 읽었으므로 실제 ISO와 대조했다. 어긋난 곳은 ADR에 반영했다.

- ISO 루트에는 `kernel`(16,278,528 B, 6.1.112-gloria-xanmod1, EFI 스텁 있음), `initrd.img`(8,492,807 B, gzip cpio),
  `install.img`(32,365,102 B, gzip cpio), `system.efs`(2,340,696,064 B), `efi/boot/android.cfg`, `isolinux/`,
  `boot/grub/`가 있다. **`system.sfs`가 아니라 `system.efs`(EROFS)다.** 그 안에 쓰기 가능한 ext4 `system.img`
  (4,718,592,000 B)가 들어 있고, initrd의 `/init`은 efs를 `/sfs`에 올린 뒤 그 안의 `system.img`를 `/android`에 올린다
  (`init` 249~254행). 대화형 설치기도 같은 `system.img`를 복사한다(`1-install` 931~940행).
- initrd의 `/init`은 조사 결과(ADR "조사 결과")와 일치한다. `AUTO_INSTALL`이면 y/n을 `read`하고(125~134행),
  busybox의 `fdisk`와 `mke2fs`를 지우며(159~160행), `INSTALL`일 때만 `install.img`를 `cpio -iud`로 루트에 풀고
  (386~388행), `/scripts/*`와 `/src/scripts/*`를 이름순으로 `source`한 뒤(391~393행) `INSTALL`이면 `do_install`을
  부른다(417행). 커널 명령줄의 `KEY=VALUE`는 환경 변수로 들어온다(`INSTALL`, `SRC`, `DEBUG`가 그렇게 온다).
- initrd에는 busybox 둘(데비안판, 도커판)과 ntfs-3g뿐이다. `install.img`에는 `sgdisk`, `cfdisk`, `gdisk`, `sfdisk`,
  `dialog`, `pv`, `mksquashfs`, grub, rEFInd가 있고 `mke2fs`는 없다. 설치기가 쓰는 `mkfs.ext3`, `tune2fs`, `e2fsck`,
  `rsync`는 ISO의 안드로이드 시스템(`/system/bin`, `/system/system_ext/bin`)에서 온다. 도우미 부팅에서 확인한 경로:
  `sgdisk=/bin/sgdisk pv=/bin/pv mke2fs=/system/bin/mke2fs mkfs.ext3=/system/bin/mkfs.ext3
  mkfs.ext4=/system/bin/mkfs.ext4 rsync=/system/system_ext/bin/rsync`.
- 설치기가 디스크에 쓰는 것은 `kernel`, `initrd.img`, `system.img`와 빈 `data/` 디렉터리뿐이다(`android-<VER>/`
  폴더 아래). ESP와 GRUB은 펌웨어가 디스크를 부팅하게 하려고 쓰는 것이라, 호스트가 커널을 직접 올리면 필요 없다.

## 2. 도우미 부팅(무인 설치)

`99-ome-install`(이 폴더)을 `scripts/99-ome-install`로 담은 newc cpio를 gzip해 `initrd.img` 뒤에 이어 붙이고
(`initrd-install.img`, 8,494,684 B), 새 32 GB qcow2와 읽기 전용 ISO를 달아 화면 없이 띄웠다. 명령줄은
`install-a.txt` 첫 줄에 있다. 핵심 인자는 `-kernel kernel -initrd initrd-install.img -append "root=/dev/ram0
console=ttyS0 OME_INSTALL=1 OME_DISK=/dev/vda OME_SRC=ome" -display none -serial file:install-serial.log`이고
`INSTALL`, `AUTO_INSTALL`, `DEBUG`는 주지 않았다. 스크립트는 `/init`이 ISO를 `/mnt`에 올리고 `/system`을 이은 뒤
source되며, 자기가 `install.img`를 풀어 `sgdisk`를 얻고, GPT에 ext4 파티션 하나를 만들어 `/system/bin/mke2fs -t
ext4`로 포맷하고, `kernel`, `initrd.img`, `/sfs/system.img`를 `/hd/ome/`에 복사한 뒤(`pv -n`으로 백분율 보고)
`data/`를 만들고 `poweroff -f`한다.

| 시각 | 사건 |
|---|---|
| 08:49:29 | QEMU 시작 |
| 08:49:3x | `OME-INSTALL start`, `efi=64`(OVMF 아래 `-kernel` 직접 부팅이 WHPX에서 된다), 도구 확인, 파티션, 포맷 |
| 08:49:4x~08:50:01 | `step copy 15 → 90`, `step finish 92`, `OME-INSTALL done` |
| 08:50:01 | 전원 꺼짐, QEMU 종료 코드 0 |

**설치 32초.** qcow2는 4,752,408,576 B가 됐다. 같은 ISO를 대화형 설치기로 OCR과 키로 넘기던 완료 실행
(`dod-local.md` 28~31회차)의 S1.5는 249~266초였다. 시리얼 로그 전체는 `install-serial.log`, OME 줄만 보면 `install-a.txt`.

## 3. 설치된 디스크의 직접 커널 부팅

ISO에서 꺼낸 `kernel`과 원본 `initrd.img`로 `-kernel kernel -initrd initrd.img -append "root=/dev/ram0 SRC=/ome quiet
console=ttyS0 console=tty0"`, 제품과 같은 표시·입력·네트워크 인자(`boot-a.txt` 첫 줄)로 부팅했다. ISO는 달지 않았다.

| 회 | QEMU 시작 → `sys.boot_completed` | 확인한 것 |
|---|---|---|
| a(첫 부팅) | 35.4초 | `ro.dalvik.vm.native.bridge=libndk_translation.so`, `/`는 `/dev/loop0`(system.img), `/data`는 `/dev/block/vda1`, `/sys/firmware/efi` 있음, `wm size` 1280x800, `adb root`가 "restarting adbd as root". `/sdcard/ome-spike.txt`를 썼다 |
| b(두 번째) | 22.8초 | `/sdcard/ome-spike.txt`가 남아 있다(a는 QMP `system_powerdown`에 안드로이드가 응답하지 않아 90초 뒤 강제 종료했는데도). `adb reboot -p`로 0.9초 만에 QEMU가 끝났다 |

제품의 끄기 순서(adb `reboot -p` → QMP → 강제)는 그대로 쓰면 된다. QMP `system_powerdown`만으로는 끝나지 않는 것은
GRUB 경로와 같다.

## 4. GRUB 경로와의 비교

29회차(`dod-home-29`, 대화형 설치기로 ESP와 GRUB을 깐 디스크, 이미 여러 번 부팅한 것)를 그 게스트의 `efivars.fd`
사본과 함께 같은 조건으로 부팅했다. QEMU 시작 → `sys.boot_completed` **26.0초**(`grub-29.txt`). 직접 부팅의 두 번째
부팅 22.8초와 견주면 3초쯤 빠르고, 그보다 GRUB 메뉴(굳음이 났던 자리, `embedded-display-freeze.md`), ESP, NVRAM
부팅 항목이 없어지는 것이 이득이다. 첫 시도는 런처의 실수(PowerShell 매개변수 `$Disk`를 뒤의 `$disk` 대입이 덮어씀)로
스파이크 디스크를 29회차 변수로 띄워 EFI 셸에 떨어졌고, 고친 뒤 다시 쟀다.

SeaBIOS는 재지 않았다. OVMF 아래 직접 부팅이 되고 제품이 이미 OVMF를 검증했으므로 바꿀 이유가 없다.

## 5. ADR에 반영한 것

- `system.sfs` → `system.efs`(안에 `system.img`). 복사 대상은 같다.
- 파티션은 GPT에 ext4 하나, ESP 없음. 폴더는 `/ome`.
- 진행 보고는 시리얼(`OME-INSTALL step <단계> <백분율>`, `done`, `fail <이유>`). 디스크 표식 파일은 쓰지 않는다.
- 펌웨어는 OVMF 그대로. 설치된 게스트의 `-append`는 `root=/dev/ram0 SRC=/ome` 뒤에 프로필의 `boot_args`.
- 도우미 부팅의 메모리는 2 GiB, vCPU 2면 충분하다.

## 파일

| 파일 | 내용 |
|---|---|
| `unattended-install-spike/99-ome-install` | 도우미 스크립트(제품에는 `host/crates/ome-guest-install/scripts/`로 들어간다) |
| `unattended-install-spike/spike.ps1` | 스파이크 런처(install, boot, grub 단계) |
| `unattended-install-spike/install-serial.log` | 도우미 부팅의 시리얼 로그 전체 |
| `unattended-install-spike/install-a.txt` | 도우미 부팅의 명령줄과 OME 줄 요약 |
| `unattended-install-spike/boot-a.txt`, `boot-b.txt` | 직접 부팅 a, b의 명령줄, 시간, adb 확인 |
| `unattended-install-spike/grub-29.txt` | GRUB 경로 비교 부팅 |
