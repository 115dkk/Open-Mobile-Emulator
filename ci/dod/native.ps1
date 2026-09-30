# SPDX-License-Identifier: GPL-2.0-or-later
[CmdletBinding()]
param(
 [int]$AppPid, [int]$QemuPid, [string]$Shot,
 [int]$Width=0, [int]$Height=0, [int]$X=80, [int]$Y=60,
 [switch]$Quit, [switch]$Inventory, [string]$HomePath,
 [string]$Keys, [string]$Text, [string]$FilePath, [switch]$GameSelection,
 [int]$KillOwnedPid, [string]$ExpectedCreation, [switch]$WaitInstaller, [switch]$Desktop, [switch]$Close,
 [switch]$Foreground, [switch]$Highlight
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
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h,StringBuilder s,int c);
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
 [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h,uint flags);
 [DllImport("user32.dll")] public static extern short VkKeyScan(char ch);
 [DllImport("user32.dll")] public static extern uint MapVirtualKey(uint code,uint map);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int cmd);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 public static void Focus(IntPtr h) {
  var root=GetAncestor(h,2);
  // Already foreground: touch nothing. Restoring or re-raising the owner would also re-band its owned
  // guest popup, and any host-side style change freezes that popup's GL presentation (ADR-0009 7번).
  if(GetForegroundWindow()==root) return;
  ShowWindow(root,9); SetForegroundWindow(root);
  for(int attempt=0;attempt<3 && GetForegroundWindow()!=root;attempt++) {
   SetWindowPos(root,new IntPtr(-1),0,0,0,0,0x43);
   try {
    System.Threading.Thread.Sleep(250);
    R r; GetWindowRect(root,out r); SetCursorPos(r.L+Math.Min(400,(r.Rt-r.L)/2),r.T+30);
    var down=new INPUT {type=0,u=new UNION {mi=new MOUSEINPUT {flags=2}}};
    var up=new INPUT {type=0,u=new UNION {mi=new MOUSEINPUT {flags=4}}};
    if(SendInput(2,new[]{down,up},Marshal.SizeOf<INPUT>())!=2) throw new Exception("Title-bar click failed");
    System.Threading.Thread.Sleep(180);
   } finally {SetWindowPos(root,new IntPtr(-2),0,0,0,0,0x53);}
  }
  if(GetForegroundWindow()!=root) throw new Exception("Main window is not foreground; refusing input/capture");
 }
 public static void Key(ushort vk,bool up=false) {
  uint flags=8u | (up?2u:0u);
  if((vk>=0x21 && vk<=0x28) || vk==0x2e) flags|=1;
  var input=new INPUT {type=1,u=new UNION {ki=new KEYBDINPUT {scan=(ushort)MapVirtualKey(vk,0),flags=flags}}};
  if(SendInput(1,new[]{input},Marshal.SizeOf<INPUT>())!=1) throw new Exception("SendInput failed");
 }
 public static void Tap(ushort vk) {Key(vk);System.Threading.Thread.Sleep(180);Key(vk,true);System.Threading.Thread.Sleep(220);}
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
if ($Desktop) {
 # Diagnostics when the product has no main window yet (CI run 16: CDP never answered, app output empty):
 # every top-level window of the app process and of its children, plus the whole virtual screen.
 Add-Type -AssemblyName System.Windows.Forms
 $family = @($AppPid) + @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $AppPid" | ForEach-Object { [int]$_.ProcessId })
 $windows = @([W]::All() | Where-Object { $family -contains [W]::Pid($_) } | ForEach-Object {
  $r=[W+R]::new();[void][W]::GetWindowRect($_,[ref]$r)
  $t=[Text.StringBuilder]::new(512);[void][W]::GetWindowText($_,$t,512)
  @{hwnd=$_.ToInt64();pid=[W]::Pid($_);class=[W]::Class($_);title=$t.ToString();visible=[W]::IsWindowVisible($_);rect=$r}
 })
 $bounds=[System.Windows.Forms.SystemInformation]::VirtualScreen
 if($Shot){
  $b=[Drawing.Bitmap]::new($bounds.Width,$bounds.Height);$g=[Drawing.Graphics]::FromImage($b)
  try{$g.CopyFromScreen($bounds.Left,$bounds.Top,0,0,$b.Size);$b.Save($Shot,[Drawing.Imaging.ImageFormat]::Png)}finally{$g.Dispose();$b.Dispose()}
 }
 $processes=@(Get-CimInstance Win32_Process -Filter "ProcessId = $AppPid OR ParentProcessId = $AppPid" | ForEach-Object { @{pid=[int]$_.ProcessId;name=$_.Name;command=$_.CommandLine} })
 @{action='desktop';screen="$($bounds.Width)x$($bounds.Height)";foreground=[W]::GetForegroundWindow().ToInt64();windows=$windows;processes=$processes;shot=$Shot} | ConvertTo-Json -Depth 6 -Compress
 exit
}
if ($Highlight) {
 # GRUB gfxmenu geometry from the capture: the selection bar is a wide run of blue pixels, the entry
 # rows are the blue lotus or chevron icons (white on the selected row) in the column left of the
 # text. Both scale with the window, so nothing here is in pixels of one display (CI run 32 lost one
 # Down and booted the wrong entry; docs/evidence/M2/dod-ci.md 32회).
 Add-Type -ReferencedAssemblies ([System.Drawing.Bitmap].Assembly.Location) -TypeDefinition @'
using System; using System.Collections.Generic; using System.Drawing; using System.Drawing.Imaging; using System.Runtime.InteropServices;
public static class G {
 static bool Blue(byte r, byte g, byte b) { return b >= 120 && b > r + 50 && b > g + 20; }
 static bool White(byte r, byte g, byte b) { return r > 200 && g > 200 && b > 200; }
 static void Push(List<double> entries, List<int> c, int h) { int span = c[c.Count-1] - c[0]; double center = (c[0] + c[c.Count-1]) / 2.0; if (span >= 6 && span <= 0.1 * h && center < 0.9 * h) entries.Add(center); }
 public static string Analyze(Bitmap bmp) {
  int w = bmp.Width, h = bmp.Height;
  var data = bmp.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
  try {
   int stride = data.Stride; var buf = new byte[stride * h]; Marshal.Copy(data.Scan0, buf, 0, buf.Length);
   var barRows = new List<int>(); int x0 = (int)(0.25 * w), x1 = (int)(0.75 * w);
   for (int y = 0; y < h; y += 2) { int n = 0, m = 0; for (int x = x0; x < x1; x += 4) { m++; int o = y * stride + x * 4; if (Blue(buf[o+2], buf[o+1], buf[o])) n++; } if (m > 0 && n * 2 > m) barRows.Add(y); }
   var clusters = new List<List<int>>();
   foreach (var y in barRows) { if (clusters.Count > 0 && y - clusters[clusters.Count-1][clusters[clusters.Count-1].Count-1] <= 4) clusters[clusters.Count-1].Add(y); else clusters.Add(new List<int>{y}); }
   List<int> big = null; foreach (var c in clusters) if (big == null || c.Count > big.Count) big = c;
   double bar = big == null ? -1 : (big[0] + big[big.Count-1]) / 2.0; int barHeight = big == null ? 0 : big[big.Count-1] - big[0];
   int b0 = (int)(0.19 * w), b1 = (int)(0.24 * w); var pres = new bool[h];
   for (int y = 0; y < h; y++) { int n = 0; for (int x = b0; x < b1; x += 2) { int o = y * stride + x * 4; if (Blue(buf[o+2], buf[o+1], buf[o]) || White(buf[o+2], buf[o+1], buf[o])) n++; } pres[y] = n >= 2; }
   var entries = new List<double>(); List<int> cur = null;
   for (int y = 0; y < h; y++) { if (!pres[y]) continue; if (cur != null && y - cur[cur.Count-1] <= 3) cur.Add(y); else { if (cur != null) Push(entries, cur, h); cur = new List<int>{y}; } }
   if (cur != null) Push(entries, cur, h);
   int row = -1; if (bar >= 0 && entries.Count > 0) { double best = double.MaxValue; for (int i = 0; i < entries.Count; i++) { double d = Math.Abs(entries[i] - bar); if (d < best) { best = d; row = i; } } if (best > 0.03 * h) row = -1; }
   var inv = System.Globalization.CultureInfo.InvariantCulture;
   return "{\"bar\":" + bar.ToString(inv) + ",\"barHeight\":" + barHeight + ",\"entries\":[" + string.Join(",", entries.ConvertAll(e => ((int)e).ToString(inv))) + "],\"row\":" + row + "}";
  } finally { bmp.UnlockBits(data); }
 }
}
'@
}
$p=Get-Process -Id $AppPid
$h=$p.MainWindowHandle
if($h -eq 0){throw 'No main window'}
$main=$h
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
 # Report the window only. Keys are sent after the caller has recognized the GRUB menu; the product
 # forwards keys to the guest now, and keys during the firmware phase are not harmless.
 @{pid=[W]::Pid($h);hwnd=$h.ToInt64();action='installer-window'} | ConvertTo-Json -Compress
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
 [W]::Focus($main)
 $map=@{HOME=0x24;END=0x23;UP=0x26;DOWN=0x28;LEFT=0x25;RIGHT=0x27;ENTER=0x0d;TAB=9;ESC=0x1b;BACKSPACE=8;SPACE=0x20}
 if($Keys){foreach($key in $Keys.Split(',')){
  if(!$map.ContainsKey($key)){throw "Unknown key $key"}
  [W]::Focus($main); [W]::Tap($map[$key])
 }}
 if($Text){[W]::Focus($main);[W]::Type($Text)}
 @{action='keys';hwnd=$h.ToInt64();pid=[W]::Pid($h);keys=$Keys;text=$Text} | ConvertTo-Json -Compress
 exit
}
if($Foreground){
 # Bring the main window to the foreground for the product's key gate and report what holds the
 # foreground afterwards without refusing: on the hosted runner another window briefly takes it back
 # (CI run 28, hwnd 918110 once), and the screenshot after each key is the check that matters.
 $settled=$false; $until=[DateTime]::UtcNow.AddSeconds(3)
 do {
  try { [W]::Focus($main) } catch { }
  Start-Sleep -Milliseconds 150
  if([W]::GetForegroundWindow() -eq $main){ $settled=$true; break }
 } until([DateTime]::UtcNow -gt $until)
 $fg=[W]::GetForegroundWindow()
 $title=[Text.StringBuilder]::new(256); [void][W]::GetWindowText($fg,$title,256)
 # Sample for a moment: a window that takes the foreground between keys makes the product's gate
 # drop the key (CI runs 30 and 32 lost single keys with the main window reported foreground).
 $others=@{}
 for($i=0;$i -lt 12;$i++){
  Start-Sleep -Milliseconds 50
  $now=[W]::GetForegroundWindow()
  if($now -ne $main -and -not $others.ContainsKey($now.ToInt64())){
   $t=[Text.StringBuilder]::new(256); [void][W]::GetWindowText($now,$t,256)
   $others[$now.ToInt64()]=@{class=[W]::Class($now);pid=[W]::Pid($now);title=$t.ToString()}
  }
 }
 @{action='foreground';settled=$settled;main=$main.ToInt64();foreground=$fg.ToInt64();foregroundClass=[W]::Class($fg);foregroundPid=[W]::Pid($fg);foregroundTitle=$title.ToString();others=$others} | ConvertTo-Json -Depth 4 -Compress
 exit
}
if($Close){
 # WM_CLOSE to the main window: the product's CloseRequested handler stops the guest and exits
 # (close_action = stopGuest), the same as a person clicking the title-bar close button.
 [void][W]::PostMessage($main,0x10,[IntPtr]0,[IntPtr]0)
 @{action='window-close';hwnd=$main.ToInt64()} | ConvertTo-Json -Compress
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
if ($QemuPid) {
 # A guest-window capture is a screen copy of an owned popup that sits above the main window; it needs no
 # foreground change, and raising the owner would re-band the popup and freeze its presentation.
 $focus='guest capture; no focus change'
} else {
 [W]::Focus($main)
 $focus='main-window foreground; no SDL focus or input attachment'
 Start-Sleep -Milliseconds 600
 if ([W]::GetForegroundWindow() -ne $main) { throw 'Foreground changed; refusing capture' }
}
$r=[W+R]::new();[void][W]::GetWindowRect($h,[ref]$r)
$crop=[W+R]::new();$dwm=if($QemuPid){[void][W]::GetWindowRect($h,[ref]$crop);0}else{[W]::DwmGetWindowAttribute($h,9,[ref]$crop,16)}
if($dwm -ne 0){throw "DWM frame query failed $dwm"}
$children=@([W]::Children($h) | ForEach-Object {$cr=[W+R]::new();[void][W]::GetWindowRect($_,[ref]$cr);@{hwnd=$_.ToInt64();pid=[W]::Pid($_);class=[W]::Class($_);visible=[W]::IsWindowVisible($_);rect=$cr}})
$highlight=$null
if($Shot){
 $b=[Drawing.Bitmap]::new(($crop.Rt-$crop.L),($crop.B-$crop.T));$g=[Drawing.Graphics]::FromImage($b)
 try{
  $g.CopyFromScreen($crop.L,$crop.T,0,0,$b.Size);$b.Save($Shot,[Drawing.Imaging.ImageFormat]::Png)
  if($Highlight){$highlight=[G]::Analyze($b) | ConvertFrom-Json}
 }finally{$g.Dispose();$b.Dispose()}
}
@{hwnd=$h.ToInt64();rect=$r;crop=$crop;focus=$focus;foreground=[W]::GetForegroundWindow().ToInt64();children=$children;shot=$Shot;highlight=$highlight} | ConvertTo-Json -Depth 6 -Compress
