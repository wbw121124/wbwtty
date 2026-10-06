<#
.SYNOPSIS
  本机会话环境（所有路径按**项目所在盘符**探测，不写死盘符）。
.DESCRIPTION
  用法（点号引入，避免子 shell）：
      . .\scripts\env.ps1
  之后 `cargo` / `rustup` / `pkg-config` / `gcc` 均可用。
  约定：
  - 临时/下载目录：$env:WBWTTY_TEMP = <项目盘符>:\wbwtty-temp
  - 环境变量一律「项目盘符优先、旧机器固定路径兜底」的候选探测：
      RUSTUP_HOME  <盘符>:\rustup  → D:\rustup
      CARGO_HOME   <盘符>:\cargo   → D:\cargo   （都不存在则不设置，用 ~/.rustup ~/.cargo）
      MSYS2        <盘符>:\msys\ucrt64\bin → E:\吴邦玮\项目\mymsys2\ucrt64\bin → D:\msys\ucrt64\bin
    候选不存在则自动跳过（CI runner / 换机都不会误设）。
#>
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$repoDrive = (Split-Path -Qualifier $repoRoot)   # 例如 F:

# 临时目录：项目同盘 <盘符>:\wbwtty-temp
$env:WBWTTY_TEMP = Join-Path "$repoDrive\" 'wbwtty-temp'
if (-not (Test-Path $env:WBWTTY_TEMP)) {
    New-Item -ItemType Directory -Path $env:WBWTTY_TEMP | Out-Null
}

# 候选探测助手：取第一个存在的路径，全无则 $null
function Get-FirstExisting([string[]]$Candidates) {
    @($Candidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1)
}

# Rust：项目盘符优先，D: 为旧机器兜底；都没有则保持默认（~/.rustup、~/.cargo）
$rustupHome = Get-FirstExisting @((Join-Path $repoDrive '\rustup'), 'D:\rustup')
if ($rustupHome) { $env:RUSTUP_HOME = $rustupHome }
$cargoHome = Get-FirstExisting @((Join-Path $repoDrive '\cargo'), 'D:\cargo')
if ($cargoHome) { $env:CARGO_HOME = $cargoHome }
$cargoBin = Get-FirstExisting @((Join-Path $repoDrive '\cargo\bin'), 'D:\cargo\bin')
if ($cargoBin) { $env:Path = "$cargoBin;" + $env:Path }
elseif (Test-Path (Join-Path $env:USERPROFILE '.cargo\bin')) {
    $env:Path = (Join-Path $env:USERPROFILE '.cargo\bin') + ';' + $env:Path
}

# MSYS2 ucrt64：项目盘符优先，旧机器路径兜底
$msysBin = Get-FirstExisting @(
    (Join-Path $repoDrive '\msys\ucrt64\bin'),
    'E:\吴邦玮\项目\mymsys2\ucrt64\bin',
    'D:\msys\ucrt64\bin'
)
if ($msysBin) { $env:Path = "$msysBin;" + $env:Path }

# 需要走代理时（pacman / 非直连下载）再取消注释：
# $env:HTTP_PROXY = 'http://127.0.0.1:7890'
# $env:HTTPS_PROXY = 'http://127.0.0.1:7890'

Write-Host "env: repo=$repoRoot" -ForegroundColor Green
Write-Host "env: temp=$env:WBWTTY_TEMP msys=$msysBin rustup=$env:RUSTUP_HOME cargo=$env:CARGO_HOME" -ForegroundColor Green
