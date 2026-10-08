# Play 스토어의 트릭컬 업데이트 검증

2026-10-08, 설치된 OME의 Android 13 게스트, WHPX·virgl, RTX 2080 SUPER.

## 결과

Google Play 스토어에서 트릭컬 업데이트를 실제 내려받아 설치했다.
기존 `10644`가 `10655`로 바뀌었고 설치 출처는 `com.android.vending`이다.
계정과 기존 게임 데이터는 유지했다. 새 설치가 아니라 기존 설치의 업데이트 시험이다.

- [업데이트 뒤 Play 스토어](trickcal-play-store-updated.png)
- [게스트 속성과 설치 출처](runtime.json)

## 확인한 원인과 수정

1. APK의 `aapt2 dump badging` 결과는 `uses-gl-es: '0x30002'`와
   `GL_KHR_texture_compression_astc_ldr`를 요구한다.
2. 실제 `SurfaceFlinger`는 GLES 3.2 Mesa 24.0.8과 해당 ASTC 확장을 제공한다.
3. 그러나 `ro.opengles.version`은 `196608`, 패키지 관리자의 기능 보고는 `0x30000`이었다.
   Bliss의 `init.sh`는 `FORCE_GLES` 인자가 없을 때 3.0을 보고한다.
4. 프로필의 부팅 인자에 `FORCE_GLES=3.2`를 넣고 재부팅하자 속성이 `196610`, 기능 보고가
   `0x30002`로 바뀌었다. 해당 변경을 저장소와 설치된 OME의 Android 13 프로필에 반영했다.
5. Play 스토어 캐시만 승인받아 초기화한 뒤 호환 불가 안내가 사라지고 Update 버튼이 나타났다.
   그 버튼으로 업데이트를 완료했다. Play 스토어 계정·설정·게임 데이터는 초기화하지 않았다.

Google은 [OpenGL ES 요구 사항](https://developer.android.com/develop/ui/views/graphics/opengl/about-opengl)과
[텍스처 압축 형식](https://developer.android.com/guide/topics/manifest/supports-gl-texture-element)을
스토어의 기기 필터링에 사용한다. 이 시험은 실제 렌더러가 지원하는 능력을 정확히 보고하도록 고친 것이다.

## 기기 프로필과 인증의 구분

공개 진단 자료의 S26 Ultra 모델명과 지문을 적용했다. 그것만 바꾼 상태에서는 트릭컬의 호환 불가
안내가 계속 나왔고, Play Protect의 인증 복구 검사도 실패했다. GLES 보고 수정과 캐시 갱신 뒤
스토어 업데이트가 성공했다. 따라서 종전 문서의 '비인증 기기이므로 트릭컬 다운로드가 불가능하다'는
단정은 이 시험으로 반박됐다. S26 지문이 이번 성공에 필수였는지는 별도의 대조 시험을 하지 않았다.

모든 파티션의 제품 식별값까지 함께 바꾼 초기 시험에서는 ANGLE 안에서 SurfaceFlinger가 충돌했다.
백업한 속성 파일 5개를 원본 SHA-256과 대조해 복원한 뒤, 공개 모델명과 빌드 지문만 바꾸는 범위로
좁혔다. 이 범위에서는 부팅과 Mesa 그래픽이 정상이다. SDK·ABI·하드웨어 코드명은 유지했다.

기기 인증 통과, 인앱 결제와 새 계정의 첫 로그인은 이 업데이트 시험이 입증하는 범위 밖이다.
게임 실행·데이터 다운로드·로비는 2026-10-09의 컴퓨터 유즈 검증으로 이어진다.
