<#
.SYNOPSIS
  本机会话环境（路径按仓库所在盘符自动探测，不写死盘符）。
.DESCRIPTION
  用法（点号引入，避免子 shell）：
      . .\scripts\env.ps1
  之后 `cargo` / `rustup` / `pkg-config` / `gcc` 均可用。
  约定：
  - 临时/下载目录：$env:WBWTTY_TEMP = <仓库所在盘符>:\wbwtty-temp
  - Rust：默认用 ~/.rustup、~/.cargo；仅当旧机器固定目录（D:\rustup、D:\cargo）
    存在时才沿用（兼容 CI runner / 旧机器，不存在则完全不动）
  - MSYS2：按候选路径探测（本机 E:\吴邦玮\项目\mymsys2，旧机器 D:\msys）
#>
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$repoDrive = (Split-Path -Qualifier $repoRoot)   # 例如 G:

# 临时目录：仓库同盘 <盘符>:\wbwtty-temp
$env:WBWTTY_TEMP = Join-Path "$repoDrive\" 'wbwtty-temp'
if (-not (Test-Path $env:WBWTTY_TEMP)) {
    New-Item -ItemType Directory -Path $env:WBWTTY_TEMP | Out-Null
}

# Rust：默认位置已在 PATH / 注册表；仅旧机器 D: 目录存在时才固定
if (Test-Path 'D:\rustup') { $env:RUSTUP_HOME = 'D:\rustup' }
if (Test-Path 'D:\cargo') { $env:CARGO_HOME = 'D:\cargo' }
if (Test-Path 'D:\cargo\bin') { $env:Path = 'D:\cargo\bin;' + $env:Path }
elseif (Test-Path (Join-Path $env:USERPROFILE '.cargo\bin')) {
    $env:Path = (Join-Path $env:USERPROFILE '.cargo\bin') + ';' + $env:Path
}

# MSYS2 ucrt64：按候选路径探测，取第一个存在的
$msysBin = @(
    'E:\吴邦玮\项目\mymsys2\ucrt64\bin',
    'D:\msys\ucrt64\bin'
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($msysBin) { $env:Path = "$msysBin;" + $env:Path }

# 需要走代理时（pacman / 非直连下载）再取消注释：
# $env:HTTP_PROXY = 'http://127.0.0.1:7890'
# $env:HTTPS_PROXY = 'http://127.0.0.1:7890'

Write-Host "env: repo=$repoRoot" -ForegroundColor Green
Write-Host "env: temp=$env:WBWTTY_TEMP msys=$msysBin rustup=$env:RUSTUP_HOME cargo=$env:CARGO_HOME" -ForegroundColor Green
