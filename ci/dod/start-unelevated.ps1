# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
# Starts the product with a UAC-style filtered token (LUA token, medium integrity) even when the
# caller is elevated. GitHub's hosted Windows runners run every job as an administrator with UAC
# off, and WebView2 150+ leaves the DevTools remote-debugging endpoint unreachable for a host that
# runs at high integrity (MicrosoftEdge/WebView2Feedback#5640): CI runs 16 and 17 saw the product
# window but no CDP port. Users never run the product elevated, so the completion run should not.
# The child inherits this process's environment (OME_HOME and the WebView2 arguments are set by the
# driver on this helper), its stdout and stderr go to -OutputFile, and this helper prints one JSON
# line with the pid, then waits and prints one JSON line with the exit code.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$FilePath,
    [Parameter(Mandatory)][string]$WorkingDirectory,
    [Parameter(Mandatory)][string]$OutputFile
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class Lua {
 [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] public struct STARTUPINFO {
  public int cb; public string lpReserved, lpDesktop, lpTitle; public int dwX, dwY, dwXSize, dwYSize, dwXCountChars, dwYCountChars, dwFillAttribute, dwFlags;
  public short wShowWindow, cbReserved2; public IntPtr lpReserved2, hStdInput, hStdOutput, hStdError; }
 [StructLayout(LayoutKind.Sequential)] public struct PROCESS_INFORMATION { public IntPtr hProcess, hThread; public int dwProcessId, dwThreadId; }
 [StructLayout(LayoutKind.Sequential)] public struct SID_AND_ATTRIBUTES { public IntPtr Sid; public uint Attributes; }
 [StructLayout(LayoutKind.Sequential)] public struct LUID { public uint LowPart; public int HighPart; }
 [StructLayout(LayoutKind.Sequential)] public struct TOKEN_PRIVILEGES { public int PrivilegeCount; public LUID Luid; public uint Attributes; }
 [DllImport("advapi32.dll", SetLastError = true)] public static extern bool OpenProcessToken(IntPtr h, uint access, out IntPtr token);
 [DllImport("advapi32.dll", SetLastError = true)] public static extern bool CreateRestrictedToken(IntPtr token, uint flags, uint disableCount, IntPtr disable, uint deleteCount, IntPtr delete, uint restrictCount, IntPtr restrict, out IntPtr newToken);
 [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)] public static extern bool ConvertStringSidToSid(string sid, out IntPtr psid);
 [DllImport("advapi32.dll", SetLastError = true)] public static extern bool SetTokenInformation(IntPtr token, int infoClass, ref SID_AND_ATTRIBUTES info, int size);
 [DllImport("advapi32.dll", SetLastError = true)] public static extern bool GetTokenInformation(IntPtr token, int infoClass, out uint info, int len, out int returned);
 [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)] public static extern bool LookupPrivilegeValue(string system, string name, out LUID luid);
 [DllImport("advapi32.dll", SetLastError = true)] public static extern bool AdjustTokenPrivileges(IntPtr token, bool disableAll, ref TOKEN_PRIVILEGES state, int len, IntPtr previous, IntPtr returned);
 [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)] public static extern bool CreateProcessAsUser(IntPtr token, string app, string cmd, IntPtr pa, IntPtr ta, bool inherit, uint flags, IntPtr env, string cwd, ref STARTUPINFO si, out PROCESS_INFORMATION pi);
 [DllImport("kernel32.dll")] public static extern IntPtr GetCurrentProcess();
 [DllImport("kernel32.dll", SetLastError = true)] public static extern bool CloseHandle(IntPtr h);
 [DllImport("kernel32.dll", SetLastError = true)] public static extern uint WaitForSingleObject(IntPtr h, uint ms);
 [DllImport("kernel32.dll", SetLastError = true)] public static extern bool GetExitCodeProcess(IntPtr h, out uint code);
 [DllImport("kernel32.dll", SetLastError = true)] public static extern bool SetHandleInformation(IntPtr h, uint mask, uint flags);
 [DllImport("kernel32.dll")] public static extern IntPtr LocalFree(IntPtr h);
 public static uint Elevation(IntPtr token) { uint value; int returned; if (!GetTokenInformation(token, 20, out value, 4, out returned)) throw new Exception("GetTokenInformation " + Marshal.GetLastWin32Error()); return value; }
}
'@
function Fail($what) { throw "$what failed: Win32 error $([Runtime.InteropServices.Marshal]::GetLastWin32Error())" }

$token = [IntPtr]::Zero
if (![Lua]::OpenProcessToken([Lua]::GetCurrentProcess(), 0xF01FF, [ref]$token)) { Fail 'OpenProcessToken' }
$before = [Lua]::Elevation($token)
# SeIncreaseQuotaPrivilege is what CreateProcessAsUser asks of the caller; the restricted token is
# derived from this process's own token, so SeAssignPrimaryTokenPrivilege is not needed.
$luid = [Lua+LUID]::new()
if ([Lua]::LookupPrivilegeValue($null, 'SeIncreaseQuotaPrivilege', [ref]$luid)) {
    $privileges = [Lua+TOKEN_PRIVILEGES]::new(); $privileges.PrivilegeCount = 1; $privileges.Luid = $luid; $privileges.Attributes = 2
    [void][Lua]::AdjustTokenPrivileges($token, $false, [ref]$privileges, [Runtime.InteropServices.Marshal]::SizeOf($privileges), [IntPtr]::Zero, [IntPtr]::Zero)
}
$restricted = [IntPtr]::Zero
if (![Lua]::CreateRestrictedToken($token, 4, 0, [IntPtr]::Zero, 0, [IntPtr]::Zero, 0, [IntPtr]::Zero, [ref]$restricted)) { Fail 'CreateRestrictedToken(LUA_TOKEN)' }
$medium = [IntPtr]::Zero
if (![Lua]::ConvertStringSidToSid('S-1-16-8192', [ref]$medium)) { Fail 'ConvertStringSidToSid' }
$level = [Lua+SID_AND_ATTRIBUTES]::new(); $level.Sid = $medium; $level.Attributes = 0x20
if (![Lua]::SetTokenInformation($restricted, 25, [ref]$level, [Runtime.InteropServices.Marshal]::SizeOf($level))) { Fail 'SetTokenInformation(TokenIntegrityLevel)' }
$after = [Lua]::Elevation($restricted)

$pairs = [Environment]::GetEnvironmentVariables().GetEnumerator() | Sort-Object { [string]$_.Key } | ForEach-Object { "$($_.Key)=$($_.Value)" }
$block = [Runtime.InteropServices.Marshal]::StringToHGlobalUni((($pairs -join "`0") + "`0`0"))
$stream = [IO.FileStream]::new($OutputFile, [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::ReadWrite)
$handle = $stream.SafeFileHandle.DangerousGetHandle()
if (![Lua]::SetHandleInformation($handle, 1, 1)) { Fail 'SetHandleInformation' }
$startup = [Lua+STARTUPINFO]::new()
$startup.cb = [Runtime.InteropServices.Marshal]::SizeOf($startup)
$startup.dwFlags = 0x100
$startup.hStdInput = [IntPtr]::Zero; $startup.hStdOutput = $handle; $startup.hStdError = $handle
$info = [Lua+PROCESS_INFORMATION]::new()
try {
    $created = [Lua]::CreateProcessAsUser($restricted, $FilePath, ('"' + $FilePath + '"'), [IntPtr]::Zero, [IntPtr]::Zero, $true, 0x400, $block, $WorkingDirectory, [ref]$startup, [ref]$info)
    # $Error is PowerShell's own read-only variable; CI run 18 died here on that name after the product had
    # already started, which left an unowned product behind.
    $createError = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
} finally {
    [Runtime.InteropServices.Marshal]::FreeHGlobal($block)
    $stream.Dispose()
}
if (!$created) { throw "CreateProcessAsUser failed: Win32 error $createError" }
# The pid line comes before anything else that could fail, so the driver always owns what was started.
@{pid=$info.dwProcessId; callerElevated=[bool]$before; childElevated=[bool]$after; integrity='medium (S-1-16-8192)'} | ConvertTo-Json -Compress
[Console]::Out.Flush()
[void][Lua]::CloseHandle($info.hThread)
[void][Lua]::WaitForSingleObject($info.hProcess, 0xFFFFFFFF)
$code = [uint32]0
[void][Lua]::GetExitCodeProcess($info.hProcess, [ref]$code)
[void][Lua]::CloseHandle($info.hProcess)
@{exit=[int64]$code} | ConvertTo-Json -Compress
