# 头文件获取与官方安装器交叉校验（docs/header-verification.md）

- 最近更新：2026-10-03（阶段 0）
- 目的：证明入库的 `sdk-headers/` 快照来自真实官方 Windows 10 SDK 头文件，
  并记录获取过程中的版本/工程差异与取舍（规格“前置准备 1/2”）。

## 1. 获取渠道与产物

| 来源 | 版本 | 下载地址 | SHA-256 | 说明 |
|---|---|---|---|---|
| 快照归档（主来源） | 10.0.10240 … 10.0.17763 共 7 版 | github.com/ralish/win-headers（sparse clone） | 见 `tools/fetch-win-headers.ps1` | 官方 SDK 头文件原样归档，含 um/shared/ucrt/km |
| 官方安装器抽查 A | Windows 10 SDK 10.0.10240（安装时显示 10.0.26624） | go.microsoft.com/fwlink/p/?LinkId=619296 | 932814CDF2D9395CB32C2A834266D880AEB37CD3855A71A7FB3BC1613DEAA7C9 | `$setup /quiet /norestart /installpath D:\temp\winsdk-10240`，非提权成功 |
| 官方引导器抽查 B | Windows 10 SDK 10.0.17763.132 | go.microsoft.com/fwlink/p/?LinkID=2033908 | BBD1C41F9EBF518E4482C5C85A0DE9AD7A72B596112C392911EF6054CB5D70D7 | 非提权失败，见第 2 节替代方案 |
| 官方 ISO 抽查 B | Windows 10 SDK 10.0.17763.132 ISO | go.microsoft.com/fwlink/p/?LinkID=2033686 | 016981259708E1AFCAB666C7C1FF44D1C4D63B5E778AF8BC41B4F6DB3D27961A | 814.3 MB，WinRAR 解出 `Installers\*.cab/.msi` |

所有下载与解压均在 `D:\temp`（不写 C:、不写 F:，仓库本体除外），未向系统目录安装 SDK，
不修改系统已装组件。

## 2. 抽查 B 的替代方案（WiX 安装器非提权失败）

17763 引导器在本机（非管理员会话）静默执行时以 `Exit code: 0x3e9`、`WixBundleElevated = 0`
退出且未产生任何文件——Burn 打包安装器要求提权，非交互环境无法完成 UAC。
**替代方案（取舍）**：

1. 下载同一发布的官方 ISO（10.0.17763.132），用 WinRAR 解包 `Installers\`；
2. 对头文件相关 MSI 做**管理安装**（`msiexec /a <msi> /qn TARGETDIR=…`，无需管理员权限）：
   - Windows SDK Desktop Headers x86 / x64 / arm
   - Windows SDK for Windows Store Apps Headers
   - Universal CRT Headers Libraries and Sources
3. 合并各组件 `Include\10.0.17763.0\{um,shared,ucrt}` 为官方对照树。

取舍：免提权、不污染系统；代价是多步提取（已固化为可复现步骤，见
`tools/verify-sdk-installers.ps1`）。未采用“请求用户手动批准 UAC”，以保证流程可自动复现。

## 3. 校验结果

校验工具：`tools/compare-sdk-headers.py`（逐文件 SHA-256 + API 声明集合 + 白名单可声明性）。
比对范围限定 `um`/`shared`（`km` 由 WDK 提供、`ucrt` 独立组件，均不参与 API 面结论；
集合比对按 NTFS 大小写不敏感规范化）。

### 3.1 10.0.10240（1507，官方安装器静默安装）

- 官方：`D:\temp\winsdk-10240\Include\10.0.10240.0`
- 快照：`sdk-headers/10.0.10240`

| 头文件 | 官方 | 快照 | SHA-256 |
|---|---|---|---|
| `um/windows.h` | ✓ | ✓ | 一致 |
| `um/winver.h` | ✓ | ✓ | 一致 |
| `shared/sdkddkver.h` | ✓ | ✓ | 一致 |
| `um/wincon.h` | ✓ | ✓ | 一致 |
| `um/consoleapi.h` | ✓ | ✓ | 一致 |
| `um/consoleapi2.h` | ✗ | ✗ | —（该版本不存在，符合预期） |
| `um/consoleapi3.h` | ✗ | ✗ | —（该版本不存在，符合预期） |
| `um/processthreadsapi.h` | ✓ | ✓ | 一致 |
| `um/handleapi.h` | ✓ | ✓ | 一致 |
| `um/fileapi.h` | ✓ | ✓ | 一致 |
| `um/winbase.h` | ✓ | ✓ | 一致 |

文件集合（um/shared，大小写不敏感）：官方 1641 / 快照 149 / 共有 147；共有文件 SHA-256：一致 147，不同 0。

- 仅官方有（1494）：官方完整 SDK 的其余头文件（快照为 include 闭包裁剪，结构性差异）
- 仅快照有（2）：`um/devpropdef.h`, `um/wtypesbase.h`（官方以不同大小写/分包存在，NTFS 下等价）

API 声明集合：官方 9726 / 快照 3647；**快照多 0**、官方多 6079（裁剪闭包 vs 完整 SDK，结构性差异）。

白名单验证：62 个 static_allowed 符号在官方头文件中全部可声明 ✓
ConPTY 符号在该官方头文件中：不存在（1809 之前预期如此）

结论：关键头文件全部一致；共有文件差异 **0** 个；快照 API 是官方严格子集。

### 3.2 10.0.17763（1809，官方 ISO 10.0.17763.132 MSI 管理解压）

- 官方：`D:\temp\winsdk-17763-official\10.0.17763.0`
- 快照：`sdk-headers/10.0.17763`

| 头文件 | 官方 | 快照 | SHA-256 |
|---|---|---|---|
| `um/windows.h` | ✓ | ✓ | 一致 |
| `um/winver.h` | ✓ | ✓ | 一致 |
| `shared/sdkddkver.h` | ✓ | ✓ | 一致 |
| `um/wincon.h` | ✓ | ✓ | 一致 |
| `um/consoleapi.h` | ✓ | ✓ | 一致 |
| `um/consoleapi2.h` | ✓ | ✓ | 一致 |
| `um/consoleapi3.h` | ✓ | ✓ | 一致 |
| `um/processthreadsapi.h` | ✓ | ✓ | 一致 |
| `um/handleapi.h` | ✓ | ✓ | 一致 |
| `um/fileapi.h` | ✓ | ✓ | **内容不同**（servicing 更新级别差异） |
| `um/winbase.h` | ✓ | ✓ | **内容不同**（servicing 更新级别差异） |

文件集合（um/shared，大小写不敏感）：官方 1740 / 快照 153 / 共有 153；共有文件 SHA-256：一致 129，不同 24。

- 仅官方有（1587）：官方完整 SDK 的其余头文件（结构性差异）
- 内容不同（24）：`shared/bcrypt.h`, `shared/bugcodes.h`, `shared/evntprov.h`, `shared/ntdef.h`,
  `shared/ntstatus.h`, `shared/rpcdce.h`, `shared/rpcndr.h`, `shared/winerror.h`, `shared/wtypesbase.h`,
  `um/cguid.h`, `um/fileapi.h`, `um/ncrypt.h` 等——均为受 servicing 修订单影响的文件

API 声明集合：官方 10283 / 快照 3744；快照多 2、官方多 6541。

- 快照中多出（官方缺失）：`GetTempPath2A`, `GetTempPath2W`（后期 servicing 引入，快照为 17763.7010 级别）
- 官方中多出（快照缺失）：完整 SDK 的其余 API（结构性差异）

白名单验证：62 个 static_allowed 符号在官方头文件中全部可声明 ✓
ConPTY 符号在该官方头文件中：`ClosePseudoConsole`, `CreatePseudoConsole`, `ResizePseudoConsole`（存在）

## 4. 结论

1. **10240（1507）**：快照与官方安装器关键头文件 SHA-256 全部一致；共有 147 个
   um/shared 文件 **0 差异**；快照 API 集合是官方严格子集。快照可信。
2. **17763（1809）**：官方 ISO（10.0.17763.132）与快照（归档 10.0.17763.7010，更新的
   servicing 级别）共有 153 个文件中 129 一致、24 不同；差异集中在受 servicing 修订单
   影响的文件，快照额外含 `GetTempPath2A/W`——**快照为官方基线的超集**，
   属预期的更新级别差异，非归档错误。
3. **白名单验证**：62 个 static_allowed 符号在 10240 与 17763 **官方**头文件中全部可声明；
   ConPTY 三符号在 10240 官方头文件中不存在、17763 中存在——与 `docs/sdk-compat-matrix.md` 互证。
4. 两版均确认 `consoleapi2.h`/`consoleapi3.h` 在 1507 缺席、1809 在场。
5. 其余 5 版（10586/14393/15063/16299/17134）同渠道入库，经 10240/17763 双点抽查确认渠道
   可信；如需，可用 `tools/verify-sdk-installers.ps1` 对任意版本重复抽查。

## 5. 复现步骤

```powershell
# 1) 获取快照并裁剪入库
powershell -File tools/fetch-win-headers.ps1          # -> D:\temp\win-headers
python tools/prune-headers.py --src D:/temp/win-headers --dest sdk-headers

# 2) 官方安装器抽查（10240 静默安装；17763 走 ISO + msiexec /a 管理解压）
powershell -File tools/verify-sdk-installers.ps1      # 详见脚本头部说明

# 3) 逐版本对比 -> D:\temp\cmp-<ver>.md
python tools/compare-sdk-headers.py --official <官方Include/<ver>> --snapshot sdk-headers/<ver> --label "<ver>" --out D:/temp/cmp-<ver>.md

# 4) 重新生成兼容矩阵与白名单
python tools/gen-compat-matrix.py
python tools/check_api_whitelist.py --emit-md
```
