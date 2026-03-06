# 下载最新版本
$url = "https://github.com/zylzyqzz/DimClaw/releases/latest/download/dimclaw-windows-x86_64.exe"
$out = "$env:USERPROFILE\dimclaw.exe"
Invoke-WebRequest -Uri $url -OutFile $out

# 添加到 PATH
$path = [Environment]::GetEnvironmentVariable("Path", "User")
if ($path -notlike "*$env:USERPROFILE*") {
    [Environment]::SetEnvironmentVariable("Path", "$path;$env:USERPROFILE", "User")
}

Write-Host "DimClaw 已安装到 $out"
Write-Host "重新打开终端后，运行 'dimclaw server' 启动服务"
