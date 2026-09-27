<!-- SPDX-License-Identifier: GPL-2.0-or-later -->
<!-- Copyright (C) 2026 Open Mobile Emulator contributors -->

# GitHub 호스트 윈도우 러너에서 WHPX가 동작하는가

측정일은 2026-09-28 02:45 KST(UTC 2026-09-27 17:45)다. `.github/workflows/probe-whpx.yml`을 손으로
띄워 `windows-2022`와 `windows-2025` 러너에서 호스트 사실을 읽고, Windows 하이퍼바이저 플랫폼
기능을 재시작 없이 켜 보고, Chocolatey의 배포판 QEMU 11.1.0(`v11.1.0-12130-ge470268ff4`, 개발
호스트에 설치된 것과 같은 빌드)으로 빈 가상 머신을 띄웠다. 실행 기록은
https://github.com/115dkk/Open-Mobile-Emulator/actions/runs/36338084070 이다.

| 항목 | windows-2022 | windows-2025 |
|---|---|---|
| OS | Windows Server 2022 Datacenter | Windows Server 2025 Datacenter |
| CPU | AMD EPYC 7763 | AMD EPYC 9V74 |
| `HypervisorPresent` | True | True |
| `VirtualizationFirmwareEnabled` | True | True |
| `HypervisorPlatform` 기능 | 이미 Enabled | 이미 Enabled |
| `VirtualMachinePlatform` 기능 | Enabled | Enabled |
| `hypervisorlaunchtype` | Auto | Auto |
| `WinHvPlatform.dll`, `WinHvEmulation.dll` | 있음 | 있음 |
| `dism /enable-feature HypervisorPlatform /norestart` | 종료 코드 0 | 종료 코드 0 |
| `-accel whpx` | 실패: `WHPX: Failed to enable nested virtualization, hr=80370302` | 8초 뒤에도 실행 중 |
| `-accel whpx,kernel-irqchip=off` | 8초 뒤에도 실행 중 | 8초 뒤에도 실행 중 |
| `-accel tcg` | 실행 중 | 실행 중 |

빈 머신은 `-machine q35 -cpu Skylake-Client-v4 -m 256 -smp 2 -display none -nodefaults`로 띄웠고,
`-accel` 값만 바꿨다. 가속기를 못 열면 QEMU는 즉시 종료하므로 "8초 뒤에도 실행 중"이 초기화 성공의
표지다.

## 판정

- 두 러너 모두 하이퍼바이저가 켜진 채 부팅되고 WHPX 기능이 이미 활성이라 재시작이 필요 없다.
  러너는 관리자 권한에 UAC가 꺼져 있어 제품의 활성화 단계는 곧바로 통과한다.
- 제품이 쓰는 `-accel whpx,kernel-irqchip=off`는 두 러너에서 다 열린다. `windows-2022`에서 plain
  `whpx`가 실패한 까닭은 QEMU가 게스트에 중첩 가상화를 노출하려다(`hr=80370302`) 거부당한 것이며,
  `kernel-irqchip=off`에서는 그 단계를 지나간다.
- 따라서 M2 완료 기준(깨끗한 기기에서 마법사만으로)을 GitHub 호스트 윈도우 러너에서 돌리는 것을
  막는 하이퍼바이저 쪽 이유는 없다. 남은 조건은 공개 저장소의 러너 사양(4 vCPU, 16 GB, C: 14 GB.
  비공개는 2 vCPU, 8 GB), ANGLE 위 virgl 또는 소프트웨어 렌더링(러너에 GPU가 없다), 러너 데스크톱
  세션에서의 창 생성, 그리고 트릭컬 APK를 CI에 둘 수 없다는 점(R1)이다.

GitHub 문서는 러너의 중첩 가상화를 "기술적으로는 가능하나 공식 지원은 아니다"라고만 적는다
(https://docs.github.com/en/actions/using-github-hosted-runners/using-github-hosted-runners/about-github-hosted-runners).
이 측정은 그 날의 러너 이미지에서 얻은 사실이며, 이미지가 바뀌면 다시 재야 한다.
