# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
# Windows PowerShell 5.1 provides the built-in WinRT projection used by Windows OCR.
# API: https://learn.microsoft.com/en-us/uwp/api/windows.media.ocr.ocrengine
[CmdletBinding()]
param([Parameter(Mandatory)][string]$Image)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
Add-Type -AssemblyName System.Runtime.WindowsRuntime
[void][Windows.Storage.StorageFile,Windows.Storage,ContentType=WindowsRuntime]
[void][Windows.Graphics.Imaging.BitmapDecoder,Windows.Graphics.Imaging,ContentType=WindowsRuntime]
[void][Windows.Graphics.Imaging.BitmapTransform,Windows.Graphics.Imaging,ContentType=WindowsRuntime]
[void][Windows.Graphics.Imaging.BitmapInterpolationMode,Windows.Graphics.Imaging,ContentType=WindowsRuntime]
[void][Windows.Graphics.Imaging.BitmapPixelFormat,Windows.Graphics.Imaging,ContentType=WindowsRuntime]
[void][Windows.Graphics.Imaging.BitmapAlphaMode,Windows.Graphics.Imaging,ContentType=WindowsRuntime]
[void][Windows.Graphics.Imaging.ExifOrientationMode,Windows.Graphics.Imaging,ContentType=WindowsRuntime]
[void][Windows.Graphics.Imaging.ColorManagementMode,Windows.Graphics.Imaging,ContentType=WindowsRuntime]
[void][Windows.Media.Ocr.OcrEngine,Windows.Foundation,ContentType=WindowsRuntime]
[void][Windows.Globalization.Language,Windows.Globalization,ContentType=WindowsRuntime]
$asTask = [System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
    $_.Name -eq 'AsTask' -and $_.IsGenericMethod -and $_.GetGenericArguments().Count -eq 1 -and
    $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
} | Select-Object -First 1
function Wait-WinRt {
    param($Operation, [Type]$ResultType)
    $task = $asTask.MakeGenericMethod($ResultType).Invoke($null, @($Operation))
    if (!$task.Wait(20000)) { throw 'OCR operation timed out' }
    return $task.Result
}
$Image = [IO.Path]::GetFullPath($Image).Replace('/', '\')
$file = Wait-WinRt ([Windows.Storage.StorageFile]::GetFileFromPathAsync($Image)) ([Windows.Storage.StorageFile])
$stream = Wait-WinRt ($file.OpenAsync([Windows.Storage.FileAccessMode]::Read)) ([Windows.Storage.Streams.IRandomAccessStream])
try {
    $decoder = Wait-WinRt ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream)) ([Windows.Graphics.Imaging.BitmapDecoder])
    # The guest window is 872x608 logical pixels in the product's stage. At 200 % display scaling (the
    # development PC) that is 1744x1216 physical pixels and the ISO GRUB menu reads cleanly; at 100 %
    # (the CI runner, CI run 25) the same menu is half the size and the OCR returned only its footer.
    # Recognize an integer-upscaled copy (Fant interpolation) so a capture is at least about 1600 px
    # wide, within the engine's maximum. The saved screenshot stays the unscaled screen copy.
    $width = [int]$decoder.PixelWidth; $height = [int]$decoder.PixelHeight
    $maximum = [int][Windows.Media.Ocr.OcrEngine]::MaxImageDimension
    $wanted = [int][Math]::Ceiling(1600 / [Math]::Max($width, 1))
    $fits = [int][Math]::Min([Math]::Floor($maximum / [Math]::Max($width, 1)), [Math]::Floor($maximum / [Math]::Max($height, 1)))
    $scale = [Math]::Max(1, [Math]::Min($wanted, $fits))
    $transform = [Windows.Graphics.Imaging.BitmapTransform]::new()
    $transform.ScaledWidth = [uint32]($width * $scale); $transform.ScaledHeight = [uint32]($height * $scale)
    $transform.InterpolationMode = [Windows.Graphics.Imaging.BitmapInterpolationMode]::Fant
    $bitmap = Wait-WinRt ($decoder.GetSoftwareBitmapAsync(
        [Windows.Graphics.Imaging.BitmapPixelFormat]::Bgra8, [Windows.Graphics.Imaging.BitmapAlphaMode]::Premultiplied, $transform,
        [Windows.Graphics.Imaging.ExifOrientationMode]::IgnoreExifOrientation, [Windows.Graphics.Imaging.ColorManagementMode]::DoNotColorManage)) ([Windows.Graphics.Imaging.SoftwareBitmap])
    try {
        $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromLanguage([Windows.Globalization.Language]::new('en-US'))
        if ($null -eq $engine) { throw 'Built-in English OCR is unavailable; cannot assert installer completion' }
        $result = Wait-WinRt ($engine.RecognizeAsync($bitmap)) ([Windows.Media.Ocr.OcrResult])
        # Each recognized line with its box in the unscaled image's pixels (the union of its words), so
        # a caller can aim at a button it reads (the guest's notification permission prompt, dev PC
        # round 25). The engine gives boxes per word only.
        $lines = @($result.Lines | ForEach-Object {
            $x0 = [double]::MaxValue; $y0 = [double]::MaxValue; $x1 = 0.0; $y1 = 0.0
            foreach ($word in $_.Words) {
                $box = $word.BoundingRect
                if ($box.X -lt $x0) { $x0 = $box.X }
                if ($box.Y -lt $y0) { $y0 = $box.Y }
                if (($box.X + $box.Width) -gt $x1) { $x1 = $box.X + $box.Width }
                if (($box.Y + $box.Height) -gt $y1) { $y1 = $box.Y + $box.Height }
            }
            @{text=$_.Text; x=[int][Math]::Floor($x0 / $scale); y=[int][Math]::Floor($y0 / $scale); w=[int][Math]::Ceiling(($x1 - $x0) / $scale); h=[int][Math]::Ceiling(($y1 - $y0) / $scale)}
        })
        @{text=$result.Text; lines=$lines; language=$engine.RecognizerLanguage.LanguageTag; width=$width; height=$height; scale=$scale} | ConvertTo-Json -Compress -Depth 4
    } finally { $bitmap.Dispose() }
} finally { $stream.Dispose() }
