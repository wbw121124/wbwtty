<#
.SYNOPSIS
  本机会话环境（C: 重启会被清空，工具链/依赖缓存/MSYS2 全部固定在 D:）。
.DESCRIPTION
  用法（点号引入，避免子 shell）：
      . .\scripts\env.ps1
  之后 `cargo` / `rustup` / `pkg-config` / `gcc` 均可用。
  若注册表用户环境变量未被 C: 还原冲掉，可直接开新终端；本脚本是兜底。
#>
$env:RUSTUP_HOME = 'D:\rustup'
$env:CARGO_HOME = 'D:\cargo'
$env:Path = 'D:\cargo\bin;D:\msys\ucrt64\bin;' + $env:Path

# 需要走代理时（pacman / 非直连下载）再取消注释：
# $env:HTTP_PROXY = 'http://127.0.0.1:7890'
# $env:HTTPS_PROXY = 'http://127.0.0.1:7890'

Write-Host "env: RUSTUP_HOME=$env:RUSTUP_HOME CARGO_HOME=$env:CARGO_HOME" -ForegroundColor Green
