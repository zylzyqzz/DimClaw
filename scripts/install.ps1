param()

$target = "$env:USERPROFILE\.dimclaw\bin"
New-Item -ItemType Directory -Force -Path $target | Out-Null

$filename = "dimclaw-windows-x86_64.exe"
$url = "https://github.com/zylzyqzz/DimClaw/releases/latest/download/$filename"
$temp = Join-Path $target $filename
$dest = Join-Path $target "dimclaw.exe"

Write-Host "正在从 $url 下载..."
Invoke-WebRequest -Uri $url -OutFile $temp -UseBasicParsing
Move-Item -Force -Path $temp -Destination $dest

Write-Host "DimClaw 已安装到 $dest"

$answer = Read-Host "是否立即启动 Web UI? (y/n)"
if ($answer -eq "y" -or $answer -eq "Y") {
    Start-Process -FilePath $dest -ArgumentList "server"
    Start-Sleep -Seconds 1
    Start-Process "http://127.0.0.1:8080"
    Write-Host "DimClaw Web UI 已启动: http://127.0.0.1:8080"
}
