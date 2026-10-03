<#
.SYNOPSIS
  阶段 0 工具：获取 7 个 Windows 10 SDK 版本的官方头文件快照到本地临时目录。
.DESCRIPTION
  从 ralish/win-headers（Windows SDK & WDK 头文件原样归档）以 sparse + blob:none
  方式克隆以下版本的头文件树（um/shared/ucrt/km 等）：
    10.0.10240 (1507), 10.0.10586 (1511), 10.0.14393 (1607), 10.0.15063 (1703),
    10.0.16299 (1709), 10.0.17134 (1803), 10.0.17763 (1809, 基线)
  所有下载一律写入 D:\temp（不在 C:、不在 F:）。
.PARAMETER Dest
  目标目录，默认 D:\temp\win-headers
.PARAMETER Proxy
  可选 HTTP 代理（默认 http://127.0.0.1:7890，失败时自动直连重试）
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File tools\fetch-win-headers.ps1
#>
[CmdletBinding()]
param(
    [string]$Dest = 'D:\temp\win-headers',
    [string]$Repo = 'https://github.com/ralish/win-headers.git',
    [string]$Proxy = 'http://127.0.0.1:7890'
)

$ErrorActionPreference = 'Stop'

$Versions = @(
    '10.0.10240', '10.0.10586', '10.0.14393', '10.0.15063',
    '10.0.16299', '10.0.17134', '10.0.17763'
)

function Invoke-Git {
    param([string[]]$GitArgs, [string]$WorkDir)
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'   # PS5.1: git stderr 进度不应变成终止错误
    $out = & git -C $WorkDir @GitArgs 2>&1
    $code = $LASTEXITCODE
    $ErrorActionPreference = $prevEap
    if ($code -ne 0) {
        throw "git $($GitArgs -join ' ') failed: $($out -join "`n")"
    }
    return $out
}

function Clone-Headers {
    if (Test-Path (Join-Path $Dest '.git')) {
        Write-Host "[fetch] already cloned: $Dest (updating)"
        $hasProxy = $false
        if ((& git -C $Dest config --get http.proxy) -is [string]) { $hasProxy = $true }
        if (-not $hasProxy) { & git -C $Dest config http.proxy $Proxy }
        $sparseArgs = @('sparse-checkout', 'set', '--cone') + $Versions
        Invoke-Git -GitArgs $sparseArgs -WorkDir $Dest | Out-Null
        try {
            Invoke-Git -GitArgs @('pull', '--ff-only') -WorkDir $Dest | Out-Null
        } catch {
            Write-Warning "[fetch] pull failed (offline?): $($_.Exception.Message)"
        }
        Invoke-Git -GitArgs @('checkout') -WorkDir $Dest | Out-Null
        Write-Host "[fetch] done"
        return
    }

    New-Item -ItemType Directory -Force -Path $Dest | Out-Null
    Write-Host "[fetch] cloning $Repo (sparse, blob:none) -> $Dest"

    # PS 5.1: git 进度写 stderr，配合 EAP=Stop 会被误判为错误 —— 原生命令周围降级 EAP
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $cloneOk = $false
    $env:HTTPS_PROXY = $Proxy
    $env:HTTP_PROXY = $Proxy
    & git clone --filter=blob:none --no-checkout --depth 1 $Repo $Dest
    if ($LASTEXITCODE -eq 0) { $cloneOk = $true }
    Remove-Item Env:HTTPS_PROXY -ErrorAction SilentlyContinue
    Remove-Item Env:HTTP_PROXY -ErrorAction SilentlyContinue
    if (-not $cloneOk) {
        Write-Host "[fetch] proxy attempt failed, retrying without proxy"
        & git clone --filter=blob:none --no-checkout --depth 1 $Repo $Dest
        if ($LASTEXITCODE -ne 0) {
            $ErrorActionPreference = $prevEap
            throw 'clone failed (with and without proxy)'
        }
    }
    $ErrorActionPreference = $prevEap

    $sparseArgs = @('sparse-checkout', 'set', '--cone') + $Versions
    Invoke-Git -GitArgs $sparseArgs -WorkDir $Dest | Out-Null
    Invoke-Git -GitArgs @('checkout') -WorkDir $Dest | Out-Null
    Write-Host "[fetch] done"
}

function Test-Layout {
    $missing = @()
    foreach ($v in $Versions) {
        foreach ($sub in @('um', 'shared', 'ucrt')) {
            $p = Join-Path $Dest "$v\$sub"
            if (-not (Test-Path $p)) { $missing += $p }
        }
        foreach ($f in @('um\windows.h', 'um\wincon.h', 'um\consoleapi.h', 'um\winver.h', 'shared\sdkddkver.h')) {
            $p = Join-Path $Dest "$v\$f"
            if (-not (Test-Path $p)) { $missing += $p }
        }
    }
    return $missing
}

Clone-Headers
$missing = Test-Layout
if ($missing.Count -gt 0) {
    Write-Warning "[fetch] missing items:"
    $missing | ForEach-Object { Write-Warning "  $_" }
    exit 1
}
Write-Host "[fetch] verified all $($Versions.Count) SDK header trees present under $Dest"
