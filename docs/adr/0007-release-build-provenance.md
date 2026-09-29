# ADR-0007: 릴리스 바이너리는 CI에서만 빌드하고 빌드 출처 증명을 붙인다

상태: 채택, 2026-09-28(PR #1 병합). M3의 완료 기준을 바꾸므로 사용자 승인 사항이며, 이 ADR을 담은
PR의 병합을 승인으로 본다.

## 배경

OME의 목표는 소스가 공개된, 누구나 쓸 수 있는 에뮬레이터다(CLAUDE.md 0절, ADR-0006). 그런데
사용자가 받는 것은 소스가 아니라 설치기와 exe, dll이다. 소스를 공개해도 릴리스 파일이 그 소스에서
나왔다는 것을 확인할 수 없다면 의혹은 남는다.

이 간극은 오래 알려진 문제다. Ken Thompson은 1984년 튜링상 강연 "Reflections on Trusting
Trust"(CACM 27권 8호)에서 소스가 깨끗해도 컴파일러가 오염되면 바이너리가 오염된다는 것을 보였다.
실제 사고도 있었다. 2024년 xz-utils 백도어(CVE-2024-3094)에서는 페이로드가 git에 든 테스트용
압축 파일에 숨어 있었고, 그것을 꺼내 빌드에 넣는 m4 스크립트는 릴리스 tarball에만 있었다. git
저장소를 읽은 사람은 이 스크립트를 볼 수 없었다.

지금 OME에서 릴리스로 나갈 바이너리 가운데 커스텀 QEMU는 개발 PC의 MSYS2(`C:\msys64`)에서
빌드한다(`qemu-build/`, CLAUDE.md 9절). R4의 `qemu-source-offer`는 소스 묶음을 보장하지만, 릴리스에 올린
바이너리가 그 묶음에서 나왔다는 것은 보장하지 않는다.

## 결정

1. 릴리스에 올리는 실행 파일(설치기, 제품 exe, 커스텀 QEMU와 동봉 DLL, OVMF)과 R4 소스 묶음은
   태그 커밋에서 GitHub Actions 윈도우 러너로만 빌드한다. 개발 PC에서 만든 파일은 릴리스에 올리지
   않는다. QEMU 빌드는 `msys2/setup-msys2@v2`(`msystem: UCRT64`)로 러너에 MSYS2를 준비하고
   `qemu-build/build-qemu.sh`를 돌린다. 스크립트에 개발 PC 전용 경로 가정(저장소 경로의 공백을
   피하려는 `%LOCALAPPDATA%` junction 등)이 있으면 이때 걷어낸다.
2. 빌드 잡은 `actions/attest@v4`로 릴리스에 올리는 파일마다 빌드 출처 증명을 붙인다. 잡 권한은
   `id-token: write`, `attestations: write`, `contents: read`다. 공개 저장소이므로 GitHub 요금제와
   상관없이 쓸 수 있다(`actions/attest-build-provenance`는 v4부터 `actions/attest`를 감싼 것이다).
3. 누구든 `gh attestation verify <파일> --repo 115dkk/Open-Mobile-Emulator`로 그 파일이 이 저장소의
   어느 커밋과 워크플로에서 나왔는지 확인할 수 있다. 이 명령을 릴리스 노트와 `docs/help/`에 적는다.
4. 자체 업데이트의 minisign 서명(CLAUDE.md M2 7번)은 그대로 둔다. 서명은 배포자가 이 파일을
   냈다는 것을, 출처 증명은 이 파일이 공개된 어떤 커밋과 워크플로에서 나왔다는 것을 보인다. 둘은
   목적이 다르다.
5. 코드 서명(CLAUDE.md 8절 3번)을 하든 안 하든 이 결정은 적용한다. 서명 없이 내보내는 경우 SmartScreen
   안내 문서에 해시 대조와 함께 출처 증명 확인 절차를 적는다.

## 결과

- M3에 6번 항목이 생긴다. `qemu-source-offer` 잡은 같은 워크플로 안에서 돌고, 소스 묶음에도 출처
  증명을 붙인다.
- 출처 증명은 파일이 어디서 나왔는지를 보장할 뿐, 같은 소스로 누가 빌드해도 비트 단위로 같은
  파일이 나온다는 것(재현 가능한 빌드)까지 보장하지는 않는다. 재현 가능한 빌드는 이 ADR의 범위
  밖이며, 필요해지면 따로 연다.
- 이미 올라간 릴리스 `qemu-v11.1.1-ome1`의 두 파일(`qemu-ome-v11.1.1-c3d48b7d1e89-win64.zip`,
  `qemu-source-offer-v11.1.1-c3d48b7d1e89.tar.gz`)은 저장소의 어떤 워크플로도 만들지 않았으므로
  개발 PC 산출물이고 증명이 없다. 제품 설치기가 이 파일을 쓰기 전에 같은 커밋과 패치로 CI에서
  다시 빌드해 증명을 붙인 릴리스로 바꾼다. `m2-dod.yml`은 이 zip을 받아 SHA-256으로 대조하므로,
  다시 빌드하면 그 값도 함께 바꾼다. 2026-09-29에 패치 0004(`-display sdl,activate-on-click=off`,
  ADR-0009 6번)를 더해 같은 방식으로 올린 `qemu-v11.1.1-ome2`와, 2026-09-30에 패치 0005(`-display
  sdl,owner-window=`, QMP `x-ome-display-window`, ADR-0009 7번)를 더한 `qemu-v11.1.1-ome3`도 같은
  처지다. 개발 PC 산출물이고 증명이 없으며, `m2-dod.yml`이 받는 zip과 SHA-256만 이것으로 바뀐다.
