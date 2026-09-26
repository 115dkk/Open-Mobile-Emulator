# ADR-0002: unsafe는 윈도우 플랫폼 크레이트의 ffi 모듈에만 있다

상태: 채택, 2026-09-26.

## 배경

사용자 지시(2026-09-26): "Unsafe도 최대한 조여야 합니다. 맥타입 타우리와 UAC 앱을 분석해서
같은 방법론을 적용합시다." 두 제품의 방식은 다음과 같다.

- UAC 원격 승인기: 워크스페이스 lints가 `unsafe_code = "forbid"`, `unsafe_op_in_unsafe_fn =
  "deny"`. 윈도우 전용 크레이트(`windows-observer`, `windows-identity`, `windows-service-host`)만
  크레이트 수준에서 `deny`로 낮추고, 사설 `ffi` 모듈에 `#[allow(unsafe_code)]`를 단다. 모든
  unsafe 블록이 핸들 소유권, 포인터 수명과 정렬, 경계, 권한 불변식을 주석으로 적는다. Tauri
  껍데기와 공유 크레이트는 `#![forbid(unsafe_code)]`다.
- MacType Control Center: 서비스 워크스페이스에서 `mactype-service-platform` 하나만 unsafe를
  갖고 `#![deny(unsafe_op_in_unsafe_fn)]`, `#![deny(clippy::undocumented_unsafe_blocks)]`를
  건다. 원시 핸들, 포인터, 버퍼 길이는 크레이트 밖으로 나가지 않고 `OwnedHandle`,
  `JobObject`, 경계 검사된 읽기(`bounded_read`) 같은 소유 타입으로만 나간다. Tauri 껍데기는
  `#![forbid(unsafe_code)]`다.

## 결정

1. `host/Cargo.toml`의 `[workspace.lints.rust]`는 `unsafe_code = "forbid"`,
   `unsafe_op_in_unsafe_fn = "deny"`, `missing_debug_implementations = "warn"`이다.
   `[workspace.lints.clippy]`는 `all = warn`, `dbg_macro`, `todo`, `unimplemented` = `deny`.
2. `ome-platform-win`만 자체 `[lints]`로 `unsafe_code = "deny"`를 갖고, `src/ffi/` 아래
   모듈만 `#![allow(unsafe_code)]`를 단다. 크레이트 루트는
   `#![deny(clippy::undocumented_unsafe_blocks)]`를 걸어 모든 unsafe 블록에 `SAFETY:` 주석을
   강제한다.
3. 다른 모든 크레이트와 실행 파일은 루트 파일 첫 다섯 줄 안에 `#![forbid(unsafe_code)]`를
   둔다. 워크스페이스 lint가 이미 `forbid`지만, 파일에도 적어 누가 봐도 알게 한다.
4. 원시 `HANDLE`, `HWND`, 포인터, 버퍼 길이는 `ome-platform-win`의 공개 인터페이스에 나타나지
   않는다. 내보내는 것은 `OwnedHandle`, `JobObject`, `Child`, `WindowHandle`처럼 소멸자가
   자원을 놓는 타입과 값이다.
5. `ci/Check-UnsafeScope.ps1`가 위 규칙을 기계적으로 검사한다. (a) 주석 밖의 `unsafe` 토큰이
   `host/crates/ome-platform-win/src/ffi/` 밖의 `.rs` 파일에 있으면 실패, (b) 다른 크레이트
   루트에 `#![forbid(unsafe_code)]`가 없으면 실패, (c) 워크스페이스 lint 값이 바뀌면 실패.
   이 검사는 pre-commit 훅과 CI에 들어간다.
6. 의존성의 unsafe는 따로 보고한다. `cargo-deny`는 라이선스와 advisories만 다루고, 의존성
   그래프가 unsafe 없다고 주장하지 않는다.

## 결과

- Win32 지식과 위험이 한 크레이트의 한 디렉터리에 모인다. 검토는 그 디렉터리만 본다.
- 새 Win32 기능이 필요하면 platform 크레이트에 소유 타입을 추가하는 일이 먼저다. 호출자
  크레이트에서 `unsafe`를 쓰는 지름길은 게이트가 막는다.
- `windows` 크레이트 기능 플래그는 platform 크레이트 `Cargo.toml`에만 나타난다.
