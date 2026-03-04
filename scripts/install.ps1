Param()

$target = "$env:USERPROFILE\.dimclaw\bin"
New-Item -ItemType Directory -Force -Path $target | Out-Null

$url = "https://github.com/zylzyqzz/DimClaw/releases/latest/download/dimclaw-windows-x86_64.exe"
$dest = Join-Path $target "dimclaw.exe"

Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing
Write-Host "DimClaw 已下载到 $dest"
