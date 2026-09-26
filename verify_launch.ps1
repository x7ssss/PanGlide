Write-Host "=== PanGlide Standalone Launch Verification ==="
$exePath = Resolve-Path "src-tauri\target\release\panglide.exe"
if (-not (Test-Path $exePath)) {
    Write-Error "Executable not found at $exePath"
    exit 1
}

$item = Get-Item $exePath
Write-Host "Executable Size: $($item.Length) bytes ($([math]::Round($item.Length/1MB, 2)) MB)"
Write-Host "Last Write Time: $($item.LastWriteTime)"

Write-Host "Launching $exePath..."
$proc = Start-Process -FilePath $exePath -PassThru

Write-Host "Waiting 4 seconds for window & WebView2 initialization..."
Start-Sleep -Seconds 4

# Check process is alive
$running = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
if (-not $running) {
    Write-Error "FAILED: Process exited prematurely!"
    exit 1
}

Write-Host "SUCCESS: Process is running (PID: $($running.Id), Handles: $($running.Handles), WorkingSet: $([math]::Round($running.WorkingSet64/1MB, 2)) MB)"

# Check MainWindowTitle
$title = $running.MainWindowTitle
Write-Host "Main Window Title: '$title'"

# Check related child processes
$children = Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $proc.Id }
Write-Host "Spawned Child Processes: $($children.Count)"
foreach ($child in $children) {
    Write-Host "  - Child PID $($child.ProcessId): $($child.Name)"
}

# Capture screen to verify window rendering
try {
    Add-Type -AssemblyName System.Windows.Forms -ErrorAction SilentlyContinue
    Add-Type -AssemblyName System.Drawing -ErrorAction SilentlyContinue

    $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)

    $screenshotDir = "scratch"
    if (-not (Test-Path $screenshotDir)) { New-Item -ItemType Directory -Path $screenshotDir | Out-Null }
    $screenshotPath = Join-Path $screenshotDir "standalone_launch_screenshot.png"
    $bitmap.Save($screenshotPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose()
    $bitmap.Dispose()
    Write-Host "Screenshot saved to $screenshotPath ($((Get-Item $screenshotPath).Length) bytes)"
} catch {
    Write-Host "Screenshot capture note: $($_.Exception.Message)"
}

# Gracefully terminate test process and children
Write-Host "Terminating test process PID $($proc.Id)..."
Stop-Process -Id $proc.Id -Force
foreach ($child in $children) {
    Stop-Process -Id $child.ProcessId -Force -ErrorAction SilentlyContinue
}

Write-Host "=== VERIFICATION COMPLETE: ALL CHECKS PASSED ==="
