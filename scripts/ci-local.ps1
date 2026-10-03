<#
.SYNOPSIS
  本机等价 CI 子集（规格：提交前必须构建+测试通过；本机无 MSVC 时由 CI 承担 per-SDK 编译）。
  运行：powershell -ExecutionPolicy Bypass -File scripts\ci-local.ps1
.DESCRIPTION
  1) 必需文档存在且非空（AGENT.md/plan.md/README.md/docs/*/各模块 README）
  2) 生成物幂等：gen-compat-matrix + check_api_whitelist --emit-md 后无 git diff
  3) API 白名单 linter
  4) （若有 workspace）cargo build + cargo test
  5) （若 cl 在 PATH）用全部 7 个 SDK 头文件编译 tools/sdk-probe/probe.c
#>
[CmdletBinding()]
param([switch]$SkipBuild)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo
$fail = 0

function Step($name, [scriptblock]$fn) {
    Write-Host "== $name ==" -ForegroundColor Cyan
    try {
        & $fn
        if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) { throw "exit code $LASTEXITCODE" }
        Write-Host "-- OK: $name" -ForegroundColor Green
    } catch {
        Write-Host "-- FAIL: $name : $($_.Exception.Message)" -ForegroundColor Red
        $script:fail = 1
    }
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
    if ($missing.Count) { $missing | ForEach-Object { Write-Host "  missing: $_" }; exit 1 }
    Write-Host "  $($required.Count) docs ok"
    exit 0
}

Step 'generated artifacts idempotent' {
    python tools/gen-compat-matrix.py | Out-Null
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    python tools/check_api_whitelist.py --emit-md | Out-Null
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    git diff --exit-code -- docs/sdk-compat-matrix.md config/api-whitelist.json `
        tools/sdk-probe/probe.c docs/api-whitelist.md
    if ($LASTEXITCODE -ne 0) {
        Write-Host '  生成物与仓库不同步：请 git add 上述文件后重试'
        exit $LASTEXITCODE
    }
    exit 0
}

Step 'api whitelist linter' { python tools/check_api_whitelist.py; exit $LASTEXITCODE }

if (-not $SkipBuild) {
    Step 'cargo build/test' {
        if (-not (Test-Path 'Cargo.toml')) { Write-Host '  workspace not created yet (stage 0) — skip'; exit 0 }
        cargo build --workspace --all-targets
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        cargo test --workspace
        exit $LASTEXITCODE
    }
}

Step 'per-SDK probe compile (needs cl in PATH)' {
    $cl = Get-Command cl -ErrorAction SilentlyContinue
    if (-not $cl) { Write-Host '  cl not in PATH — skipped (CI windows runner 承担)'; exit 0 }
    foreach ($v in @('10.0.10240','10.0.10586','10.0.14393','10.0.15063','10.0.16299','10.0.17134','10.0.17763')) {
        $inc = "sdk-headers/$v"
        & cl /nologo /Zs /c 'tools/sdk-probe/probe.c' "/I$inc/um" "/I$inc/shared" "/I$inc/ucrt" "/I$inc/km" "/Foprobe-$v.obj"
        if ($LASTEXITCODE -ne 0) { Write-Host "  FAIL: SDK $v"; exit $LASTEXITCODE }
        Remove-Item "probe-$v.obj" -ErrorAction SilentlyContinue
        Write-Host "  ok: $v"
    }
    exit 0
}

if ($fail) { Write-Host 'ci-local: FAILED' -ForegroundColor Red; exit 1 }
Write-Host 'ci-local: PASSED' -ForegroundColor Green
exit 0
