# OME 입력기

호스트의 윈도우 입력기가 조합한 글자를 게스트 앱의 입력 칸에 전달하는 안드로이드 입력기입니다. 화면 자판, 런처 아이콘, 액티비티는 없습니다. 하드웨어 키는 가로채지 않습니다.

- 패키지와 서비스는 `org.openmobileemulator.ime/.OmeInputMethodService`입니다.
- 안드로이드 13(API 33) 이상에서 설치하며, 대상 API는 35입니다.
- `version.txt`의 양의 정수 한 줄이 `versionCode`입니다. `versionName`은 `<versionCode>.0`입니다.
- 자판 모드는 한국어(`ko_KR`)와 영어(`en_US`)를 등록합니다. 안드로이드의 서브타입은 로캘 하나만 받으므로 두 항목으로 적습니다. 실제 글자 조합과 언어 전환은 호스트 입력기가 담당합니다.

## 빌드

PowerShell 7, JDK 11 이상, 안드로이드 SDK의 build-tools와 API 33 이상 플랫폼이 필요합니다. Gradle이나 AndroidX는 쓰지 않습니다.

```powershell
$env:ANDROID_HOME = 'C:\Program Files (x86)\Android\android-sdk'
$env:JAVA_HOME = 'C:\Program Files\jdk-17'
& 'C:\Open Mobile Emulator\guest\ime\build.ps1'
```

SDK는 `ANDROID_HOME`, `ANDROID_SDK_ROOT`, `%LOCALAPPDATA%\Android\Sdk` 순으로 찾습니다. 설치된 build-tools 중 가장 높은 정식 버전을 사용합니다. `android-35/android.jar`가 있으면 고르고, 없으면 API 33 이상 중 가장 높은 버전으로 컴파일합니다. JDK는 `JAVA_HOME`을 먼저 보고, 없으면 PATH에서 찾습니다. 필요한 도구가 없으면 내려받지 않고 실패합니다.

빌드는 `aapt2 compile`, `aapt2 link`, `javac --release 11`, `d8`, DEX 파일 추가, `zipalign`, `apksigner` 순으로 실행합니다. 서명과 정렬을 검증한 뒤 `out/`에 세 파일을 남깁니다.

- `ome-ime.apk`
- `ome-ime.apk.sha256` (SHA-256과 APK 파일명)
- `ome-ime.json` (`versionCode`, `versionName`, `sha256`)

`out/` 전체는 Git에서 무시합니다. 키를 지정하지 않으면 매번 무작위 비밀번호로 RSA 3072 임시 키를 만들어 서명하고, 성공 여부와 관계없이 지웁니다. 따라서 같은 소스를 다시 빌드해도 서명과 APK의 SHA-256은 달라집니다. 중간 파일도 남기지 않습니다.

이미 가진 키를 쓰려면 `ome-ime` 별칭이 있는 키 저장소를 지정합니다. 비밀번호를 명령 인자로 넘기지 않습니다.

```powershell
$env:OME_IME_STORE_PASSWORD = '<키 저장소 비밀번호>'
$env:OME_IME_KEY_PASSWORD = '<개인 키 비밀번호>'
& 'C:\Open Mobile Emulator\guest\ime\build.ps1' -KeyStore 'C:\Keys\ome-ime.p12'
```

`OME_IME_KEY_PASSWORD`를 생략하면 키 저장소 비밀번호를 사용합니다. 지정한 키 파일은 지우지 않습니다. 임시 키로 서명한 이전 APK와 서명이 다르면 `adb install -r`로 교체할 수 없습니다. 이때는 **OME 입력기 패키지만** 제거하고 다시 설치합니다. 입력기는 데이터를 저장하지 않습니다.

## 연결과 프로토콜

프레임과 명령은 [ADR-0013의 결정 5번](../../docs/adr/0013-korean-text-input.md)을 따릅니다. 서비스의 추상 유닉스 소켓 이름은 `ome-ime`입니다.

```powershell
adb install -r 'C:\Open Mobile Emulator\guest\ime\out\ome-ime.apk'
adb shell ime enable org.openmobileemulator.ime/.OmeInputMethodService
adb shell ime set org.openmobileemulator.ime/.OmeInputMethodService
adb forward tcp:0 localabstract:ome-ime
```

마지막 명령이 반환한 포트에 호스트가 TCP로 접속합니다. 접속 직후 `hello`를 받고, 활성 입력 칸이 있으면 이어서 `focus`를 받습니다. 새 접속은 이전 접속을 종료합니다. 소켓에서 읽고 쓰는 작업은 백그라운드 스레드에서, 입력 칸에 명령을 적용하는 작업은 메인 스레드에서 합니다.

추상 소켓 이름 자체에는 접근 권한이 없으므로 피어 UID가 셸(2000)이나 root(0)인지 확인합니다. 일반 앱 UID의 연결은 거절합니다. 네트워크 권한은 요청하지 않습니다. 프레임은 최대 65,536 UTF-16 코드 단위로 제한하고, 초과하면 연결을 종료합니다. 대기열은 128개로 제한합니다. 잘못된 JSON과 모르는 명령은 무시합니다. 포커스가 바뀌기 전에 받은 대기 명령은 새 입력 칸에 적용하지 않습니다.

## 편집 키와 순서

Backspace와 Delete는 선택 영역을 지우거나 커서 앞뒤의 유니코드 코드 포인트 하나를 지웁니다. 조합 중이면 조합 문자열의 끝(Backspace)이나 처음(Delete)에서 코드 포인트 하나를 빼고 조합을 갱신합니다. 조합을 먼저 확정하지 않습니다.

Left, Right, Home, End는 조합을 확정한 뒤 추출한 편집 상태로 커서를 옮깁니다. Left와 Right는 서로게이트 쌍을 나누지 않습니다. Home과 End는 추출한 텍스트의 처음과 끝으로 이동하며, 문서 앞부분이 빠진 추출 결과로는 Home을 처리하지 않습니다. 한 명령 안의 연산은 `beginBatchEdit`와 `endBatchEdit`로 묶습니다.

여러 줄 입력 칸의 Enter는 줄바꿈 문자열을 확정합니다. 키 이벤트와 글자 입력이 뒤섞이지 않도록 하기 위해서입니다. 한 줄 입력 칸은 지정된 편집 액션이 있으면 실행하고, 없으면 Enter 키 이벤트를 보냅니다.

Up, Down, Tab, Escape는 조합을 확정한 뒤 키 이벤트로 전달합니다. **이 네 키와 글자 명령을 빠르게 섞어 보내면 게스트의 처리 순서를 보장하지 못합니다.** 편집 상태를 추출하지 못해 방향키로 대신 처리하거나, 삭제할 글자를 읽을 수 없거나 없어서 삭제 키로 대신 처리하는 경우에도 같은 한계가 있습니다. 한 줄 입력 칸의 액션 없는 Enter도 키 이벤트이므로 같은 제한을 받습니다.

로그 태그는 `OmeIme`입니다. 글자 원문과 JSON 원문을 남기지 않고, 조합·확정 명령의 글자 수만 기록합니다.

시험 후 원래 입력기로 돌리는 명령은 다음과 같습니다.

```powershell
adb shell ime set com.android.inputmethod.latin/.LatinIME
```
