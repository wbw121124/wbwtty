<#
.SYNOPSIS
  阶段 0 工具：官方 Windows 10 SDK 安装器下载/提取 + 头文件交叉校验（可复现脚本）。
.DESCRIPTION
  抽查 A：10.0.10240 安装器静默安装到 <临时目录>\winsdk-10240（非提权可用）。
  抽查 B：10.0.17763 引导器在非提权环境以 0x3e9 失败 —— 改用官方 ISO，
          WinRAR 解包 Installers\，再以 msiexec /a（管理安装，无需管理员）
          提取 Desktop Headers / Store Headers / UCRT Headers 并合并为
          <临时目录>\winsdk-17763-official\10.0.17763.0。
  随后调用 tools/compare-sdk-headers.py 逐版本对比，产出 <临时目录>\cmp-<ver>.md。
  所有下载/解压只写临时目录（默认 <仓库盘符>:\wbwtty-temp，按脚本位置探测）。
.NOTES
  下载与解压产物（安装器/ISO）SHA-256 记录于 docs/header-verification.md。
  幂等：已存在的文件不会重复下载/解压。
#>
[CmdletBinding()]
param(
    [string]$Work = '',
    [string]$Repo = '',
    [string]$Proxy = 'http://127.0.0.1:7890',
    [switch]$SkipDownload
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$repoDrive = (Split-Path -Qualifier $repoRoot)
if (-not $Work) { $Work = Join-Path "$repoDrive\" 'wbwtty-temp' }
if (-not $Repo) { $Repo = $repoRoot }
$dl = Join-Path $Work 'winsdk-dl'
New-Item -ItemType Directory -Force -Path $dl | Out-Null

function Get-File {
    param([string]$Url, [string]$Out)
    if (Test-Path $Out) { Write-Host "[verify] cached: $Out"; return }
    Write-Host "[verify] downloading $Url -> $Out"
    try {
        Invoke-WebRequest -Uri $Url -OutFile $Out -Proxy $Proxy -TimeoutSec 1800 -UseBasicParsing
    } catch {
        Write-Warning "[verify] proxy failed ($($_.Exception.Message)); retrying direct"
        Invoke-WebRequest -Uri $Url -OutFile $Out -TimeoutSec 1800 -UseBasicParsing
    }
}

function Invoke-MsiAdminExtract {
    param([string]$Msi, [string]$Target)
    if (-not (Test-Path $Msi)) { throw "missing msi: $Msi" }
    New-Item -ItemType Directory -Force -Path $Target | Out-Null
    $p = Start-Process msiexec -ArgumentList @('/a', "`"$Msi`"", '/qn', '/norestart', "TARGETDIR=$Target") -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "msiexec /a failed ($($p.ExitCode)): $Msi" }
    Write-Host "[verify] extracted: $Msi -> $Target"
}

# ---------- 抽查 A：10.0.10240 ----------
$a = Join-Path $dl 'sdk10240-setup.exe'
if (-not $SkipDownload) { Get-File 'https://go.microsoft.com/fwlink/p/?LinkId=619296' $a }
$inc10240 = Join-Path $Work 'winsdk-10240\Include\10.0.10240.0'
if (-not (Test-Path $inc10240)) {
    Write-Host '[verify] silent-installing 10.0.10240 to D:\temp\winsdk-10240'
    $p = Start-Process $a -ArgumentList @('/quiet', '/norestart', '/installpath', (Join-Path $Work 'winsdk-10240')) -Wait -PassThru
    Write-Host "[verify] 10240 setup exit=$($p.ExitCode)"
}
if (-not (Test-Path $inc10240)) { throw "10240 headers not found at $inc10240" }

# ---------- 抽查 B：10.0.17763（ISO + MSI 管理解压） ----------
$b = Join-Path $dl 'sdk17763-setup.exe'
$iso = Join-Path $dl 'sdk17763.iso'
if (-not $SkipDownload) {
    Get-File 'https://go.microsoft.com/fwlink/p/?LinkID=2033908' $b
    Get-File 'https://go.microsoft.com/fwlink/p/?LinkID=2033686' $iso
}
$isoDir = Join-Path $Work 'winsdk-17763-iso'
$official17763 = Join-Path $Work 'winsdk-17763-official\10.0.17763.0'
if (-not (Test-Path (Join-Path $isoDir 'Installers'))) {
    Write-Host '[verify] extracting ISO with WinRAR'
    $wr = @('C:\Program Files (x86)\WinRAR\WinRAR.exe', 'C:\Program Files\WinRAR\WinRAR.exe') |
        Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $wr) { throw 'WinRAR not found (needed to unpack ISO without admin)' }
    New-Item -ItemType Directory -Force -Path $isoDir | Out-Null
    $p = Start-Process $wr -ArgumentList @('x', '-ibck', '-o+', $iso, "$isoDir\") -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "WinRAR extraction failed: $($p.ExitCode)" }
}
if (-not (Test-Path $official17763)) {
    $inst = Join-Path $isoDir 'Installers'
    $components = @(
        'Windows SDK Desktop Headers x86-x86_en-us.msi',
        'Windows SDK Desktop Headers x64-x86_en-us.msi',
        'Windows SDK Desktop Headers arm-x86_en-us.msi',
        'Windows SDK for Windows Store Apps Headers-x86_en-us.msi',
        'Universal CRT Headers Libraries and Sources-x86_en-us.msi'
    )
    $stage = Join-Path $Work 'msi-stage-17763'
    foreach ($c in $components) {
        $dest = Join-Path $stage ($c -replace '[^A-Za-z0-9]', '-')
        Invoke-MsiAdminExtract -Msi (Join-Path $inst $c) -Target $dest
    }
    New-Item -ItemType Directory -Force -Path $official17763 | Out-Null
    foreach ($d in (Get-ChildItem $stage -Directory)) {
        $src = Join-Path $d.FullName 'Windows Kits\10\Include\10.0.17763.0'
        if (Test-Path $src) {
            robocopy $src $official17763 /E /XO /NFL /NDL /NJH /NJS | Out-Null
            if ($LASTEXITCODE -ge 8) { throw "robocopy failed: $src" }
        }
    }
    Write-Host "[verify] merged official tree: $official17763"
}

# ---------- 对比 ----------
python (Join-Path $Repo 'tools\compare-sdk-headers.py') `
    --official $inc10240 --snapshot (Join-Path $Repo 'sdk-headers\10.0.10240') `
    --label '10.0.10240 (1507, 官方安装器 10.0.26624 静默安装)' `
    --out (Join-Path $Work 'cmp-10240.md')
if ($LASTEXITCODE -ne 0) { throw 'compare 10.0.10240 failed' }

python (Join-Path $Repo 'tools\compare-sdk-headers.py') `
    --official $official17763 --snapshot (Join-Path $Repo 'sdk-headers\10.0.17763') `
    --label '10.0.17763 (1809, 官方 ISO 10.0.17763.132 MSI 管理解压)' `
    --out (Join-Path $Work 'cmp-17763.md')
if ($LASTEXITCODE -ne 0) { throw 'compare 10.0.17763 failed' }

Write-Host '[verify] done: D:\temp\cmp-10240.md, D:\temp\cmp-17763.md'
Write-Host '[verify] 人工汇总结果到 docs/header-verification.md（第 3 节）'
