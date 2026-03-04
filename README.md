## 项目简介
DimClaw 是一个本地优先、多智能体协作的执行框架，原生兼容 OpenClaw 技能格式。

## 本地运行
```
cargo build
cargo run -- submit --title "测试" --command "echo hello"
cargo run -- run --once
cargo run -- list
```

## 发布说明
将 `cargo build --release` 产出的 `target/release/dimclaw` 复制到部署目录或系统服务。

## 安装命令
Linux/macOS: `curl -fsSL https://github.com/zylzyqzz/DimClaw/releases/latest/download/dimclaw-linux-x86_64 -o ~/.local/bin/dimclaw && chmod +x ~/.local/bin/dimclaw`
Windows PowerShell: `Invoke-WebRequest https://github.com/zylzyqzz/DimClaw/releases/latest/download/dimclaw-windows-x86_64.exe -OutFile $env:USERPROFILE\.dimclaw\bin\dimclaw.exe`
