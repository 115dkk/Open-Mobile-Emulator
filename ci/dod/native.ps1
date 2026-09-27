# SPDX-License-Identifier: GPL-2.0-or-later
[CmdletBinding()]
param(
 [int]$AppPid, [int]$QemuPid, [string]$Shot,
 [int]$Width=0, [int]$Height=0, [int]$X=80, [int]$Y=60,
 [switch]$Quit, [switch]$Inventory, [string]$HomePath,
 [string]$Keys, [string]$Text, [string]$FilePath, [switch]$GameSelection,
 [int]$KillOwnedPid, [string]$ExpectedCreation, [switch]$WaitInstaller
)
$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
if ($Inventory -or $KillOwnedPid) {
 $processes = @(Get-CimInstance Win32_Process -Filter "Name = 'ome.exe' OR Name LIKE 'qemu-system%'" |
  ForEach-Object { @{pid=[int]$_.ProcessId; parent=[int]$_.ParentProcessId; name=$_.Name;
   path=$_.ExecutablePath; command=$_.CommandLine; creation=$_.CreationDate.ToUniversalTime().ToString('o')} })
 if ($KillOwnedPid) {
  $owned = @($processes | Where-Object { $_.pid -eq $KillOwnedPid -and $_.creation -ceq $ExpectedCreation })
  if ($owned.Count -ne 1) { throw 'Owned process identity no longer matches; refusing to terminate' }
  Stop-Process -Id $KillOwnedPid -Force -ErrorAction Stop
 }
 $drive = if ($HomePath) { [IO.DriveInfo]::new([IO.Path]::GetPathRoot($HomePath)) } else { $null }
 @{processes=$processes; freeBytes=if($drive){$drive.AvailableFreeSpace}else{$null}; killed=$KillOwnedPid} | ConvertTo-Json -Depth 5 -Compress
 exit
}
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class W {
 [StructLayout(LayoutKind.Sequential)] public struct R {public int L,T,Rt,B;}
 public delegate bool E(IntPtr h, IntPtr p);
 [DllImport("user32.dll")] public static extern bool EnumWindows(E cb,IntPtr p);
 [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr h,E cb,IntPtr p);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out R r);
 [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h,int a,out R r,int size);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int he,uint flags);
 [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int he,bool repaint);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,StringBuilder s,int c);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern int GetMenuItemCount(IntPtr m);
 [DllImport("user32.dll")] public static extern uint GetMenuItemID(IntPtr m,int i);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetMenuString(IntPtr m,uint i,StringBuilder s,int c,uint f);
 [StructLayout(LayoutKind.Sequential)] public struct INPUT {public uint type; public UNION u;}
 [StructLayout(LayoutKind.Explicit)] public struct UNION {
  [FieldOffset(0)] public KEYBDINPUT ki; [FieldOffset(0)] public MOUSEINPUT mi;
 }
 [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT {public ushort vk,scan; public uint flags,time; public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT {public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [DllImport("user32.dll",SetLastError=true)] public static extern uint SendInput(uint n,INPUT[] inputs,int size);
 [DllImport("user32.dll")] public static extern IntPtr SetFocus(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h,uint flags);
 [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a,uint b,bool attach);
 [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
 [DllImport("user32.dll")] public static extern short VkKeyScan(char ch);
 [DllImport("user32.dll")] public static extern uint MapVirtualKey(uint code,uint map);
 [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int cmd);
 public static void Focus(IntPtr h) {
  var root=GetAncestor(h,2); ShowWindow(root,9);
  uint p, current=GetCurrentThreadId();
  uint foreground=GetWindowThreadProcessId(GetForegroundWindow(),out p);
  uint target=GetWindowThreadProcessId(h,out p);
  if(foreground!=current) AttachThreadInput(current,foreground,true);
  if(target!=current && target!=foreground) AttachThreadInput(current,target,true);
  // Keep input queues attached until this short-lived helper exits. Detaching before
  // SendInput restores webview focus for a cross-process reparented SDL child.
  BringWindowToTop(root);SetForegroundWindow(root);SetFocus(h);
  if(GetForegroundWindow()!=root) throw new Exception("Target window is not foreground; refusing keyboard input or desktop capture");
 }
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 public static void ClickFocus(IntPtr h) {
  R r; GetWindowRect(h,out r); SetCursorPos(r.L+20,r.T+20);
  var down=new INPUT {type=0,u=new UNION {mi=new MOUSEINPUT {flags=2}}};
  var up=new INPUT {type=0,u=new UNION {mi=new MOUSEINPUT {flags=4}}};
  if(SendInput(2,new[]{down,up},Marshal.SizeOf<INPUT>())!=2) throw new Exception("Mouse focus SendInput failed");
  System.Threading.Thread.Sleep(150);
 }
 public static void Key(ushort vk,bool up=false) {
  uint flags=up?2u:0u;
  if(vk>=0x21 && vk<=0x28) flags|=1;
  var input=new INPUT {type=1,u=new UNION {ki=new KEYBDINPUT {vk=vk,scan=(ushort)MapVirtualKey(vk,0),flags=flags}}};
  if(SendInput(1,new[]{input},Marshal.SizeOf<INPUT>())!=1) throw new Exception("SendInput failed");
 }
 public static void Tap(ushort vk) {Key(vk);System.Threading.Thread.Sleep(45);Key(vk,true);System.Threading.Thread.Sleep(90);}
 public static void Type(string text) {foreach(char c in text) {
  short k=VkKeyScan(c);if(k==-1 || (k>>8)>1) throw new Exception("Unsupported keyboard character");
  bool shift=(k&256)!=0;if(shift)Key(0x10);Tap((ushort)(k&255));if(shift)Key(0x10,true);
 }}
 public static string Class(IntPtr h) {var s=new StringBuilder(256); GetClassName(h,s,256); return s.ToString();}
 public static List<IntPtr> All() {var a=new List<IntPtr>(); EnumWindows((h,p)=>{a.Add(h); return true;},IntPtr.Zero);return a;}
 public static List<IntPtr> Children(IntPtr parent) {var a=new List<IntPtr>(); EnumChildWindows(parent,(h,p)=>{a.Add(h);return true;},IntPtr.Zero);return a;}
 public static uint Pid(IntPtr h) {uint p; GetWindowThreadProcessId(h,out p);return p;}
}
'@
[void][W]::SetThreadDpiAwarenessContext([IntPtr](-4))
$p=Get-Process -Id $AppPid
$h=$p.MainWindowHandle
if($h -eq 0){throw 'No main window'}
if ($WaitInstaller) {
 $deadline = [DateTime]::UtcNow.AddSeconds(30)
 do {
  $candidates = @([W]::All()) + @([W]::Children($h))
  $targets = @($candidates | Where-Object { [W]::Pid($_) -ne $AppPid -and [W]::Class($_) -eq 'SDL_app' -and [W]::IsWindowVisible($_) } | Select-Object -Unique)
  $targets = @($targets | Where-Object {
   $candidate = Get-CimInstance Win32_Process -Filter "ProcessId = $([W]::Pid($_))"
   $candidate.ParentProcessId -eq $AppPid -and $candidate.CommandLine.Replace('\','/').Contains($HomePath.Replace('\','/') + '/')
  })
  if ($targets.Count -eq 1) { break }
  Start-Sleep -Milliseconds 40
 } while ([DateTime]::UtcNow -lt $deadline)
 if ($targets.Count -ne 1) { throw 'Owned visible installer window did not appear' }
 $h = $targets[0]
 # Interrupt GRUB's short live-boot timeout as soon as the window is created.
 # Do not select an entry until the caller has captured and recognized the menu.
 $until = [DateTime]::UtcNow.AddSeconds(3)
 do { [W]::Focus($h); [W]::Tap(0x24); Start-Sleep -Milliseconds 100 } while ([DateTime]::UtcNow -lt $until)
 @{pid=[W]::Pid($h);hwnd=$h.ToInt64();action='interrupt-installer-grub'} | ConvertTo-Json -Compress
 exit
}
if ($QemuPid) {
 $candidates = @([W]::All()) + @([W]::Children($h))
 $targets = @($candidates | Where-Object { [W]::Pid($_) -eq $QemuPid -and [W]::Class($_) -eq 'SDL_app' -and [W]::IsWindowVisible($_) } | Select-Object -Unique)
 if($targets.Count -ne 1){throw "SDL_app window count $($targets.Count) for owned QEMU $QemuPid"}
 $h=$targets[0]
}
if ($FilePath) {
 Add-Type -AssemblyName UIAutomationClient
 Add-Type -AssemblyName UIAutomationTypes
 $until=[DateTime]::UtcNow.AddSeconds(15)
 do {
  $dialogs=@([W]::All() | Where-Object { [W]::Pid($_) -eq $AppPid -and [W]::Class($_) -eq '#32770' -and [W]::IsWindowVisible($_) })
  if(!$dialogs.Count){Start-Sleep -Milliseconds 100}
 } until($dialogs.Count -or [DateTime]::UtcNow -gt $until)
 if($dialogs.Count -ne 1){throw 'Expected one owned file dialog'}
 $dialog=$dialogs[0]; [W]::Focus($dialog)
 $element=[Windows.Automation.AutomationElement]::FromHandle($dialog)
 $combo=$element.FindFirst([Windows.Automation.TreeScope]::Descendants,
  [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::AutomationIdProperty,'1148'))
 if($null -eq $combo){throw 'File-name control 1148 not found'}
 $edit=$combo.FindFirst([Windows.Automation.TreeScope]::Subtree,
  [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::Edit))
 if($null -eq $edit){throw 'File-name edit not found'}
 $value=$edit.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)
 $selection = if ($GameSelection) {
  (@('base.apk','split_config.arm64_v8a.apk','split_gpdeku.apk','split_gpdeku.config.arm64_v8a.apk') |
   ForEach-Object { '"' + (Join-Path $FilePath $_) + '"' }) -join ' '
 } else { $FilePath }
 $value.SetValue($selection); $edit.SetFocus(); [W]::Tap(0x0d)
 $until=[DateTime]::UtcNow.AddSeconds(15)
 while([W]::IsWindowVisible($dialog) -and [DateTime]::UtcNow -lt $until){Start-Sleep -Milliseconds 100}
 if([W]::IsWindowVisible($dialog)){throw 'File dialog did not close'}
 @{action='file-dialog';closed=$true;privateSelection=[bool]$GameSelection} | ConvertTo-Json -Compress
 exit
}
if ($Keys -or $Text) {
 [W]::Focus($h)
 if ($QemuPid) { [W]::ClickFocus($h) }
 $map=@{HOME=0x24;END=0x23;UP=0x26;DOWN=0x28;LEFT=0x25;RIGHT=0x27;ENTER=0x0d;TAB=9;ESC=0x1b;BACKSPACE=8;SPACE=0x20}
 if($Keys){foreach($key in $Keys.Split(',')){
  if(!$map.ContainsKey($key)){throw "Unknown key $key"}
  [W]::Focus($h); [W]::Tap($map[$key])
 }}
 if($Text){[W]::Focus($h);[W]::Type($Text)}
 @{action='keys';hwnd=$h.ToInt64();pid=[W]::Pid($h);keys=$Keys;text=$Text} | ConvertTo-Json -Compress
 exit
}
if($Quit){
 $tray=@([W]::All() | Where-Object { [W]::Pid($_) -eq $AppPid -and [W]::Class($_) -eq 'tray_icon_app' })
 if($tray.Count -ne 1){throw "Tray window count $($tray.Count)"}
 [void][W]::PostMessage($tray[0],6002,[IntPtr]0,[IntPtr]0x205)
 $until=[DateTime]::UtcNow.AddSeconds(5)
 do {$menus=@([W]::All() | Where-Object {[W]::Pid($_) -eq $AppPid -and [W]::Class($_) -eq '#32768'}); if(!$menus.Count){Start-Sleep -Milliseconds 100}} until($menus.Count -or [DateTime]::UtcNow -gt $until)
 if(!$menus.Count){throw 'Tray popup did not appear'}
 $menu=[W]::SendMessage($menus[0],0x1e1,[IntPtr]0,[IntPtr]0)
 $items=@();$quitId=$null
 for($i=0;$i -lt [W]::GetMenuItemCount($menu);$i++){
  $s=[Text.StringBuilder]::new(256);[void][W]::GetMenuString($menu,$i,$s,256,0x400)
  $id=[W]::GetMenuItemID($menu,$i);$items+=@{label=$s.ToString();id=$id}
  if($s.ToString() -eq '종료'){$quitId=$id}
 }
 if($null -eq $quitId){throw 'No 종료 menu item'}
 [void][W]::PostMessage($tray[0],0x1f,[IntPtr]0,[IntPtr]0)
 [void][W]::PostMessage($tray[0],0x111,[IntPtr]$quitId,[IntPtr]0)
 @{action='native tray 종료';items=$items;owner=$tray[0].ToInt64()} | ConvertTo-Json -Compress
 exit
}
if($Width -gt 0){if(![W]::MoveWindow($h,$X,$Y,$Width,$Height,$true)){throw 'MoveWindow failed'}}
$foreground=[W]::SetForegroundWindow($h)
$focus='SetForegroundWindow'
if([W]::GetForegroundWindow() -ne $h){$ok=(New-Object -ComObject WScript.Shell).AppActivate($AppPid);$focus="AppActivate=$ok"}
$topmost=$false
if([W]::GetForegroundWindow() -ne $h){
 $topmost=[W]::SetWindowPos($h,[IntPtr](-1),0,0,0,0,0x43)
 $focus+="; temporary HWND_TOPMOST=$topmost"
}
[W]::Focus($h)
Start-Sleep -Milliseconds 600
if ([W]::GetForegroundWindow() -ne [W]::GetAncestor($h,2)) { throw 'Foreground changed; refusing to capture another application' }
$r=[W+R]::new();[void][W]::GetWindowRect($h,[ref]$r)
$crop=[W+R]::new();$dwm=if($QemuPid){[void][W]::GetWindowRect($h,[ref]$crop);0}else{[W]::DwmGetWindowAttribute($h,9,[ref]$crop,16)}
if($dwm -ne 0){throw "DWM frame query failed $dwm"}
$children=@([W]::Children($h) | ForEach-Object {$cr=[W+R]::new();[void][W]::GetWindowRect($_,[ref]$cr);@{hwnd=$_.ToInt64();pid=[W]::Pid($_);class=[W]::Class($_);visible=[W]::IsWindowVisible($_);rect=$cr}})
if($Shot){
 $b=[Drawing.Bitmap]::new(($crop.Rt-$crop.L),($crop.B-$crop.T));$g=[Drawing.Graphics]::FromImage($b)
 try{$g.CopyFromScreen($crop.L,$crop.T,0,0,$b.Size);$b.Save($Shot,[Drawing.Imaging.ImageFormat]::Png)}finally{$g.Dispose();$b.Dispose()}
}
if($topmost){[void][W]::SetWindowPos($h,[IntPtr](-2),0,0,0,0,0x53)}
@{hwnd=$h.ToInt64();rect=$r;crop=$crop;focus=$focus;foreground=[W]::GetForegroundWindow().ToInt64();children=$children;shot=$Shot} | ConvertTo-Json -Depth 6 -Compress
