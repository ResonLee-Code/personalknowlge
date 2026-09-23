# 在 Windows 上构建 Personal Knowledge OS 安装包。
#
# 用法（在仓库根目录执行）：
#   .\scripts\windows\build-windows.ps1              # 编译 + 打包 NSIS 安装包
#   .\scripts\windows\build-windows.ps1 -NoBundle    # 只编译 exe
#
# 前置条件：
#   - Node.js 20+ 与 pnpm
#   - Rust（MSVC 工具链，rustup 默认安装即可）
#   - 首次运行前执行过 pnpm install
param(
    [switch]$NoBundle,
    [switch]$DebugBuild
)

$ErrorActionPreference = 'Stop'

$RootDir = Resolve-Path (Join-Path $PSScriptRoot '..\..')
Set-Location $RootDir

Write-Host '==> 检查工具链' -ForegroundColor Cyan
foreach ($tool in @('node', 'pnpm', 'cargo')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "缺少依赖：$tool，请先安装后再运行。"
    }
}
cargo --version
pnpm --version

if (-not (Test-Path 'node_modules')) {
    Write-Host '==> 安装前端依赖' -ForegroundColor Cyan
    pnpm install
}

if ($NoBundle) {
    $profileFlag = if ($DebugBuild) { @() } else { @('--release') }
    Write-Host '==> 编译 Windows 可执行文件' -ForegroundColor Cyan
    Push-Location src-tauri
    cargo build @profileFlag
    Pop-Location
    $exe = if ($DebugBuild) { 'src-tauri\target\debug\personal-knowledge-os.exe' } else { 'src-tauri\target\release\personal-knowledge-os.exe' }
    Write-Host "==> 产物：$exe"
    exit 0
}

Write-Host '==> 构建前端' -ForegroundColor Cyan
pnpm build

Write-Host '==> 打包 NSIS 安装包' -ForegroundColor Cyan
pnpm tauri build --bundles nsis

$bundleDir = 'src-tauri\target\release\bundle\nsis'
if (Test-Path $bundleDir) {
    Write-Host '==> 安装包产物：' -ForegroundColor Green
    Get-ChildItem $bundleDir -Filter '*.exe' | ForEach-Object { Write-Host "    $($_.FullName)" }
} else {
    throw '未找到安装包输出目录，请检查构建日志。'
}
