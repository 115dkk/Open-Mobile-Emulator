# P3 조사 C: AOSP 빌드를 돌릴 빌드 호스트 선택지 (2026-10-01)

ASTRA 워커가 2026-10-01에 공식 문서와 가격표를 읽어 확인한 사실표다. 가격은 공식 페이지의 수치만 적었고 접근일은 모두 2026-10-01이다.

## 1. GitHub 표준 호스트 러너

| 항목 | 웹에서 확인한 사실 | 출처 |
|---|---|---|
| 공개 저장소의 `ubuntu-24.04` x64 사양 | 4 vCPU, RAM 16 GB, SSD 14 GB로 문서화. 유지관리자는 보장하는 여유 공간이 14 GB라고 밝힘 | https://docs.github.com/en/actions/reference/runners/github-hosted-runners , https://github.com/actions/runner-images/discussions/9329 |
| 공개 저장소 무료 여부 | 표준 러너의 실행 시간은 무료. 대형 러너에는 적용되지 않음 | https://docs.github.com/en/billing/concepts/product-billing/github-actions |
| 잡 실행 시간 | 잡당 상한 6시간 | https://docs.github.com/en/actions/reference/limits |
| runner-images 디스크 설정 | Packer 설정 `os_disk_size_gb = 75`(이미지 제작 설정) | https://github.com/actions/runner-images/blob/main/images/ubuntu/templates/locals.ubuntu.pkr.hcl |
| 최근 Ubuntu 24.04 실행 사례 | Trivy DB 2026-09 PR: 루트 145 GB, 정리 전 여유 87 GB(실행 사례, 보장 아님) | https://github.com/aquasecurity/trivy-db/pull/694 |
| `easimon/maximize-build-space` | 기본 여유 25~29 GB, 도구 제거로 60 GB 초과. 실험적 | https://github.com/easimon/maximize-build-space |
| Ubuntu 24.04 정리 후 최대 용량 | 확인 못 함. 250~400 GB 확보 근거 없음 | |

## 2. GitHub 대형 러너

| vCPU | RAM | SSD | 분당 요금, USD |
|---:|---:|---:|---:|
| 2 | 8 GB | 75 GB | $0.006 |
| 4 | 16 GB | 150 GB | $0.012 |
| 8 | 32 GB | 300 GB | $0.022 |
| 16 | 64 GB | 600 GB | $0.042 |
| 32 | 128 GB | 1,200 GB | $0.082 |
| 64 | 256 GB | 2,040 GB | $0.162 |

- GitHub Team 또는 Enterprise Cloud 조직·엔터프라이즈 대상. 개인 계정 저장소용 상품은 확인되지 않음. 공개 저장소에서도 유료. 6시간 잡 제한 적용.
- 출처: https://docs.github.com/en/actions/reference/runners/larger-runners , https://docs.github.com/en/billing/reference/actions-runner-pricing

## 3. 자체 호스팅 러너, OIDC와 출처 증명

- 개인 저장소에 등록 가능(저장소 소유자). Debian 10 이상 지원. WSL2 명시 없음.
- 자체 호스팅 러너 사용료 무료, 잡당 최대 5일.
- OIDC 발급자 같음(`token.actions.githubusercontent.com`), `runner_environment` 클레임이 `self-hosted`. `gh attestation verify --deny-self-hosted-runners` 옵션 존재.
- SLSA v1.0 L2는 개인 워크스테이션이 아닌 전용 인프라를 요구. 개인 PC 러너 + attest는 L2가 아님.
- 보안 경고 원문: "Self-hosted runners should almost never be used for public repositories on GitHub, because any user can open pull requests against the repository and compromise the environment."
- 출처: https://docs.github.com/en/actions/how-tos/manage-runners/self-hosted-runners/add-runners , https://docs.github.com/en/actions/reference/security/oidc , https://cli.github.com/manual/gh_attestation_verify , https://docs.github.com/en/actions/reference/security/secure-use , https://slsa.dev/spec/v1.0/levels

## 4. 다른 CI·클라우드

| 서비스 | 사양·한도 | 디스크·시간 | 출처 |
|---|---|---|---|
| Cirrus CI 공개 OSS | 월 50 credits = Linux 10,000 CPU-minutes, 작업당 8 CPU / 32 GB | 작업 최대 2시간. 디스크 확인 못 함 | https://github.com/cirruslabs/cirrus-ci-docs/blob/master/docs/faq.md |
| Oracle Always Free x86 | E2.1.Micro 2대, 대당 1/8 OCPU, 1 GB RAM | 총 200 GB | https://docs.oracle.com/en-us/iaas/Content/FreeTier/freetier_topic-Always_Free_Resources.htm |
| GitHub Codespaces | Free 120 core-hours/월, 15 GB-month | 최대 32코어 128 GB 디스크 | https://docs.github.com/codespaces/overview |

Hetzner Cloud(독일·핀란드, 2026-06-15 요금, EUR, 부가세 제외):

| 상품 | vCPU | RAM | NVMe | 시간당 | 월 상한 |
|---|---:|---:|---:|---:|---:|
| CPX42 공유 | 8 | 16 GB | 320 GB | €0.1114 | €69.49 |
| CPX52 공유 | 12 | 24 GB | 480 GB | €0.1610 | €100.49 |
| CCX33 전용 | 8 | 32 GB | 240 GB | €0.2219 | €138.49 |
| CCX43 전용 | 16 | 64 GB | 360 GB | €0.4423 | €275.99 |

- 볼륨 10 GB~10 TB, GB당 현행 단가 확인 못 함.
- 출처: https://www.hetzner.com/cloud/regular-performance/ , https://www.hetzner.com/cloud/general-purpose/ , https://docs.hetzner.com/general/infrastructure-and-availability/price-adjustment/

Scaleway `POP2-HC-16C-32G` €0.4256/h, `POP2-HC-32C-64G` €0.8512/h(디스크 별도, 블록 5K IOPS €0.000130/GB·h). OVHcloud US b3-32 $0.2415/h, b3-64 $0.4829/h(2026-10-01부터 디스크·IPv4 별도, 최종 요금 확인 못 함). 출처: https://www.scaleway.com/en/pricing/virtual-instances/ , https://us.ovhcloud.com/public-cloud/prices/

## 5. AOSP·Bliss OS 빌드 요구사항

| 문서 | RAM | 디스크 | 비고 | 출처 |
|---|---|---|---|---|
| AOSP 현행 | 최소 64 GB | 최소 여유 400 GB(checkout 250 + 빌드 150) | 72코어 64 GB 약 40분, 6코어 64 GB 약 6시간 | https://source.android.com/docs/setup/start/requirements |
| Bliss `Build bliss os 15.x` | 16 GB(VM 32 GB) | 350 GB(repo 약 170 GB) | 16코어 이상 권장, 데스크톱 4~6시간. 본문은 `arcadia-x86` manifest 안내 | https://docs.blissos.org/development/build-bliss-os-15.x/ |

- `repo init --depth=1`은 얕은 이력. manifest의 `clone-depth`가 덮어쓸 수 있음. `repo sync -c`는 현재 브랜치만.
- Android 14 r29 동기화 900 GB 사례(clone-depth 누락). AOSPLite: Android 16 r4 shallow + 제거 뒤 111 GB(빌드 성공·out 크기 아님).
- Android 15/16 Bliss foss의 소스·out 실측 확인 못 함.

## 6. WSL2

- 2021년 WSL2(RAM 20 GB) AOSP 빌드 성공 사례. Android 15/16 Bliss WSL2 성공은 확인 못 함.
- 작업은 리눅스 파일시스템에(`/mnt/c` 느림). `.wslconfig` `[wsl2]` memory 기본 50%, processors 전체, swap 25%(`%Temp%\swap.vhdx`). 적용은 `wsl --shutdown` 뒤.
- ext4 VHDX 가상 최대 1 TB(기본). `wsl --manage <distro> --resize`(WSL 2.5+). 가상 상한은 물리 여유를 늘리지 않음.
- `wsl --export/--import --vhd`, `wsl --import-in-place <이름> <vhdx>`로 다른 드라이브에 등록.
- `wsl --mount <Disk>`(관리자)는 USB/flash/SD 리더 미지원. `wsl --mount --vhd <경로>`는 Store판 WSL에서 지원.
- 출처: https://learn.microsoft.com/en-us/windows/wsl/filesystems , https://learn.microsoft.com/en-us/windows/wsl/wsl-config , https://learn.microsoft.com/en-us/windows/wsl/disk-space , https://learn.microsoft.com/en-us/windows/wsl/basic-commands , https://learn.microsoft.com/en-us/windows/wsl/wsl2-mount-disk , https://www.effie.io/post/running-and-deploying-aosp-on-windows-with-wsl2-part-2/

## 요약

- AOSP 공식 최소 RAM 64 GB, 디스크 400 GB. Bliss 문서 350 GB.
- GitHub 표준 러너는 디스크·RAM·시간 모두 부족. 대형 러너는 조직 전용이고 공개 저장소도 유료.
- 자체 호스팅 러너는 개인 저장소에서 가능하나 공개 저장소에는 보안 경고. 증명은 되지만 SLSA L2 아님.
- Hetzner CCX43(16 vCPU, 64 GB, 360 GB) €0.4423/h이 공식 최소를 채우는 가장 싼 주문형 선택지.
