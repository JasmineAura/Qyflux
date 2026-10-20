param([string]$Version)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$config = Get-Content -LiteralPath (Join-Path $projectRoot 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json
if (!$Version) { $Version = $config.version }
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'Version must be MAJOR.MINOR.PATCH.' }
if ($Version -ne $config.version) { throw 'Release version does not match tauri.conf.json.' }
$package = Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json
if ($package.version -ne $Version) { throw 'package.json version does not match release version.' }
$cargo = Get-Content -LiteralPath (Join-Path $projectRoot 'src-tauri/Cargo.toml') -Raw
if ($cargo -notmatch ('(?m)^version\s*=\s*"' + [regex]::Escape($Version) + '"\s*$')) { throw 'Cargo.toml version does not match release version.' }
$binary = Join-Path $projectRoot 'src-tauri/target/release/qyflux.exe'
if (!(Test-Path -LiteralPath $binary)) { throw 'Build the application with npm run build first.' }
# Read the PE header to avoid labeling a different architecture as x64.
$stream = [System.IO.File]::OpenRead($binary)
$reader = [System.IO.BinaryReader]::new($stream)
try {
    $stream.Position = 0x3c
    $stream.Position = $reader.ReadInt32() + 4
    if ($reader.ReadUInt16() -ne 0x8664) { throw 'The release binary must target Windows x64.' }
} finally { $reader.Dispose() }
$stem = "Qyflux-v$Version-windows-x64"
$outputDir = Join-Path $projectRoot 'dist'
$stageDir = Join-Path $outputDir $stem
if (Test-Path -LiteralPath $stageDir) { throw "Package staging folder already exists: $stageDir" }
New-Item -ItemType Directory -Path $stageDir -Force | Out-Null
Copy-Item -LiteralPath $binary -Destination (Join-Path $stageDir 'Qyflux.exe')
Copy-Item -LiteralPath (Join-Path $projectRoot 'ui/fonts/OFL.txt') -Destination (Join-Path $stageDir '0xProto-LICENSE.txt')
$instructions = @"
Qyflux v$Version - Windows x64

使用方法
1. 将 ZIP 解压到固定目录，双击 Qyflux.exe 启动。
2. Alt + Space 呼出/隐藏环形工具箱；Esc 隐藏工具箱。
3. 在主程序中配置标签与快捷方式，在桌面区域中双击打开项目。
4. 关闭主窗口后会继续驻留托盘；右键托盘图标可彻底退出。

运行环境
Windows 10 / 11 x64；需要 Microsoft Edge WebView2 Runtime。
WebView2 下载：https://developer.microsoft.com/microsoft-edge/webview2/
无需安装 Rust、Node.js，也无需下载 GitHub 的 Source code 压缩包。

数据目录：%APPDATA%/studio.qyflux.desktop/
更新前先从托盘退出旧版本，再替换程序。现有配置保存在上述目录中。
本压缩包只包含程序、说明与字体许可证，不包含作者个人配置。

项目主页：https://github.com/JasmineAura/Qyflux
问题反馈：https://github.com/JasmineAura/Qyflux/issues
"@
[System.IO.File]::WriteAllText((Join-Path $stageDir '使用说明.txt'), $instructions, [System.Text.UTF8Encoding]::new($true))
$exePath = Join-Path $outputDir "$stem.exe"
$zipPath = Join-Path $outputDir "$stem.zip"
Copy-Item -LiteralPath $binary -Destination $exePath -Force
Compress-Archive -LiteralPath (Get-ChildItem -LiteralPath $stageDir -File).FullName -DestinationPath $zipPath -CompressionLevel Optimal -Force
$checksums = foreach ($file in @($exePath, $zipPath)) {
    $hash = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([System.IO.Path]::GetFileName($file))"
}
[System.IO.File]::WriteAllLines((Join-Path $outputDir 'SHA256SUMS.txt'), $checksums, [System.Text.UTF8Encoding]::new($false))
Get-Item -LiteralPath $exePath, $zipPath, (Join-Path $outputDir 'SHA256SUMS.txt') | Select-Object Name, Length
