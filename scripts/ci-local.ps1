<#
.SYNOPSIS
  本机等价 CI 子集（规格：提交前必须构建+测试通过；本机无 MSVC 时由 CI 承担 per-SDK 编译）。
  运行：powershell -ExecutionPolicy Bypass -File scripts\ci-local.ps1
.DESCRIPTION
  1) 必需文档存在且非空（AGENT.md/plan.md/README.md/docs/*/各模块 README）
  2) 生成物幂等：gen-compat-matrix + check_api_whitelist --emit-md 后无 git diff
  3) API 白名单 linter
   4) （若有 workspace）cargo build + cargo test
   4b) pty-unix 在 x86_64-unknown-linux-gnu 目标上 cargo check（已安装该 target 时）
   5) （若 cl 在 PATH）用全部 7 个 SDK 头文件编译 tools/sdk-probe/probe.c
  步骤失败用 throw 抛出（不可用 exit —— exit 会终止整个脚本）。
#>
[CmdletBinding()]
param([switch]$SkipBuild)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo
# 路径按仓库所在盘符探测（scripts/env.ps1 的等价内联，fresh 进程自包含）：
# - 临时目录 <盘符>:\wbwtty-temp；- Rust 默认位置，仅旧机器 D: 目录存在时才固定
#   （CI runner 无 D:\rustup 时自动跳过，不影响 GitHub Actions）；- MSYS2 候选探测
$repoDrive = (Split-Path -Qualifier $repo)
$env:WBWTTY_TEMP = Join-Path "$repoDrive\" 'wbwtty-temp'
if (Test-Path 'D:\rustup') { $env:RUSTUP_HOME = 'D:\rustup' }
if (Test-Path 'D:\cargo') { $env:CARGO_HOME = 'D:\cargo' }
if (Test-Path 'D:\cargo\bin') { $env:Path = 'D:\cargo\bin;' + $env:Path }
$msysBin = @(
    'E:\吴邦玮\项目\mymsys2\ucrt64\bin',
    'D:\msys\ucrt64\bin'
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($msysBin) { $env:Path = "$msysBin;" + $env:Path }
$fail = 0

function Step($name, [scriptblock]$fn) {
    Write-Host "== $name ==" -ForegroundColor Cyan
    try {
        & $fn
        Write-Host "-- OK: $name" -ForegroundColor Green
    } catch {
        Write-Host "-- FAIL: $name : $($_.Exception.Message)" -ForegroundColor Red
        $script:fail = 1
    }
}

function Run-Tool([string]$Exe, [string[]]$ToolArgs) {
    & $Exe @ToolArgs
    if ($LASTEXITCODE -ne 0) { throw "$Exe exited with $LASTEXITCODE" }
}

Step 'docs presence' {
    $required = @('AGENT.md', 'plan.md', 'README.md',
        'docs/sdk-compat-matrix.md', 'docs/api-whitelist.md', 'docs/header-verification.md')
    foreach ($d in (Get-ChildItem 'crates' -Directory -ErrorAction SilentlyContinue)) {
        $required += (Join-Path $d.FullName 'README.md')
    }
    $missing = @()
    foreach ($f in $required) {
        if (-not (Test-Path $f) -or (Get-Item $f).Length -eq 0) { $missing += $f }
        elseif ($f -in @('AGENT.md', 'plan.md') -and
                -not (Select-String -Path $f -Pattern '最近更新' -Quiet)) {
            $missing += "$f (缺少最近更新标记)"
        }
    }
    if ($missing.Count) { $missing | ForEach-Object { Write-Host "  missing: $_" }; throw 'docs missing' }
    Write-Host "  $($required.Count) docs ok"
}

Step 'generated artifacts idempotent' {
    Run-Tool 'python' @('tools/gen-compat-matrix.py')
    Run-Tool 'python' @('tools/check_api_whitelist.py', '--emit-md')
    & git diff --exit-code -- docs/sdk-compat-matrix.md config/api-whitelist.json `
        tools/sdk-probe/probe.c docs/api-whitelist.md
    if ($LASTEXITCODE -ne 0) {
        throw '生成物与仓库不同步：请 git add docs/sdk-compat-matrix.md config/api-whitelist.json tools/sdk-probe/probe.c docs/api-whitelist.md 后重试'
    }
}

Step 'api whitelist linter' {
    & python tools/check_api_whitelist.py
    if ($LASTEXITCODE -ne 0) { throw "linter exit $LASTEXITCODE" }
}

if (-not $SkipBuild) {
    Step 'cargo build/test' {
        if (-not (Test-Path 'Cargo.toml')) {
            Write-Host '  workspace not created yet (stage 0) - skip'
            return
        }
        & cargo build --workspace --all-targets
        if ($LASTEXITCODE -ne 0) { throw "cargo build exit $LASTEXITCODE" }
        & cargo test --workspace
        if ($LASTEXITCODE -ne 0) { throw "cargo test exit $LASTEXITCODE" }
        if (Test-Path 'examples/pipe_mvp.rs') {
            & cargo run --example pipe_mvp
            if ($LASTEXITCODE -ne 0) { throw "pipe_mvp exit $LASTEXITCODE" }
        }
    }

    Step 'pty-unix cross-target check (linux)' {
        if (-not (Test-Path 'crates/pty-unix')) { Write-Host '  no pty-unix yet - skip'; return }
        $installed = @(& rustup target list --installed 2>$null) | ForEach-Object { "$_".Trim() }
        if (($installed -notcontains 'x86_64-unknown-linux-gnu')) {
            Write-Host '  linux target not installed - skipped (rustup target add x86_64-unknown-linux-gnu)'
            return
        }
        & cargo check -p pty-unix --target x86_64-unknown-linux-gnu --all-targets
        if ($LASTEXITCODE -ne 0) { throw "cross check exit $LASTEXITCODE" }
    }
}

Step 'per-SDK probe compile (needs cl in PATH)' {
    $cl = Get-Command cl -ErrorAction SilentlyContinue
    if (-not $cl) { Write-Host '  cl not in PATH - skipped (CI windows runner 承担)'; return }
    foreach ($v in @('10.0.10240','10.0.10586','10.0.14393','10.0.15063','10.0.16299','10.0.17134','10.0.17763')) {
        $inc = "sdk-headers/$v"
        & cl /nologo /Zs /c 'tools/sdk-probe/probe.c' "/I$inc/um" "/I$inc/shared" "/I$inc/ucrt" "/I$inc/km" "/Foprobe-$v.obj"
        if ($LASTEXITCODE -ne 0) { throw "SDK $v probe compile failed (cl exit $LASTEXITCODE)" }
        Remove-Item "probe-$v.obj" -ErrorAction SilentlyContinue
        Write-Host "  ok: $v"
    }
}

if ($fail) { Write-Host 'ci-local: FAILED' -ForegroundColor Red; exit 1 }
Write-Host 'ci-local: PASSED' -ForegroundColor Green
exit 0
