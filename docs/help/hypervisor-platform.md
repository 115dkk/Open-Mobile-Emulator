# Windows 하이퍼바이저 플랫폼

Open Mobile Emulator는 Windows에 들어 있는 하이퍼바이저(Hyper-V)의 API인 Windows 하이퍼바이저
플랫폼(WHPX) 위에서 가상 머신을 돌립니다. 자체 커널 드라이버를 설치하지 않으며, 이 기능이 켜져
있어야 합니다.

## 제품이 하는 일

첫 실행 마법사의 `활성화` 버튼을 누르면 제품은 관리자 권한 확인창(UAC)을 띄운 뒤 Windows의
선택적 기능 `HypervisorPlatform`을 켭니다. 이것은 `Windows 기능 켜기/끄기`에서
`Windows 하이퍼바이저 플랫폼`에 체크하는 것과 같은 일입니다. 켠 뒤에는 다시 시작해야 적용됩니다.

제품은 사용자가 이 버튼을 누를 때만 이 작업을 하고, 그 밖의 어떤 시스템 설정도 바꾸지 않습니다.
부팅 구성(`bcdedit`)은 어떤 경우에도 건드리지 않습니다.

## 알아 둘 것

- 이 기능을 켜면 Windows 자체가 하이퍼바이저 위에서 동작합니다. 커널 안티치트를 쓰는 일부 PC
  게임은 이 상태에서 실행을 거부합니다. 그런 게임을 해야 한다면 기능을 끄고 다시 시작해야 하며,
  제품은 그 설정을 대신 바꾸지 않습니다.
- VirtualBox와 VMware도 이 기능이 켜진 PC에서는 Windows 하이퍼바이저 위에서 돕니다. 함께 쓸 수
  있지만 속도가 달라질 수 있습니다.
- Windows 11 Home에도 이 기능은 있습니다. Hyper-V 관리 도구와는 다른 항목입니다.

## 직접 켜거나 끄기

1. 시작 메뉴에서 `Windows 기능 켜기/끄기`를 엽니다.
2. `Windows 하이퍼바이저 플랫폼`에 체크하거나 체크를 해제합니다.
3. 확인을 누르고 다시 시작합니다.

명령으로 하려면 관리자 PowerShell에서 다음을 실행합니다.

```
dism.exe /online /enable-feature /featurename:HypervisorPlatform /norestart
```

끌 때는 `enable-feature`를 `disable-feature`로 바꿉니다.
