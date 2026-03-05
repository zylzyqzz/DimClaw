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
