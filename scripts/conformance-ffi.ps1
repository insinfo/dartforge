param(
    [string]$DartExe = 'dart',
    [string]$ClangExe = '',
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
if (!$IsWindows) { throw 'This SDK @Native process-resolution oracle currently requires Windows; native Rust integration tests remain cross-platform.' }
. "$PSScriptRoot/env.ps1"
$root = Split-Path $PSScriptRoot
$previousClang = $env:DARTFORGE_CLANG
Push-Location $root
try {
    $dart = (Get-Command $DartExe -ErrorAction Stop).Source
    if (!$ClangExe) { $ClangExe = if ($env:DARTFORGE_CLANG) { $env:DARTFORGE_CLANG } else { 'clang' } }
    $clang = (Get-Command $ClangExe -ErrorAction Stop).Source
    $env:DARTFORGE_CLANG = $clang
    $version = (& $dart --version 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE -or $version -notmatch 'Dart SDK version: 3\.6\.2(?:\s|$)') { throw "Requires Dart 3.6.2: $version" }
    if (!$SkipBuild) {
        cargo build --release --locked -p dartforge-cli
        if ($LASTEXITCODE) { throw 'DartForge release build failed' }
    }
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE) { throw 'Cargo metadata failed' }
    $compiler = Join-Path $metadata.target_directory 'release/dartforge.exe'
    if (!(Test-Path -LiteralPath $compiler)) { throw "Missing release compiler: $compiler" }
    $runDir = Join-Path $root ('target/ffi-conformance/' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Force $runDir | Out-Null
    $source = Join-Path $root 'tests/ffi/main.dart'
    $nativeSource = Join-Path $root 'tests/ffi/native.c'
    $expected = (Get-Content -LiteralPath 'tests/ffi/main.stdout' -Raw).Replace("`r`n", "`n").TrimEnd([char[]]"`n")
    Copy-Item -LiteralPath $source -Destination (Join-Path $runDir 'main.dart')
    $dll = Join-Path $runDir 'fixture.dll'
    & $clang -shared -O2 $nativeSource -o $dll
    if ($LASTEXITCODE) { throw 'Native reference DLL compilation failed' }
    $dartPath = $dll.Replace('\', '/').Replace("'", "\'").Replace('$', '\$')
    $oracle = Join-Path $runDir 'oracle.dart'
    @"
import 'dart:ffi';
import 'main.dart' as fixture;
void main() {
  DynamicLibrary.open('$dartPath');
  fixture.main();
}
"@ | Set-Content -Encoding utf8 $oracle
    $vm = @(& $dart $oracle)
    if ($LASTEXITCODE -or ($vm -join "`n") -cne $expected) { throw 'Dart VM @Native differs from the checked-in expected output' }
    $vm | Set-Content -Encoding utf8 (Join-Path $runDir 'vm.stdout')
    $official = Join-Path $runDir 'official.exe'
    & $dart compile exe $oracle -o $official
    if ($LASTEXITCODE) { throw 'Dart AOT oracle compilation failed' }
    $aot = @(& $official)
    if ($LASTEXITCODE -or ($aot -join "`n") -cne $expected) { throw 'Dart AOT @Native differs from expected output' }
    $aot | Set-Content -Encoding utf8 (Join-Path $runDir 'aot.stdout')
    $results = @()
    foreach ($optimized in @($false, $true)) {
        $level = if ($optimized) { 'O2' } else { 'O0' }
        $object = Join-Path $runDir ("fixture.$level.obj")
        & $clang -c "-$level" $nativeSource -o $object
        if ($LASTEXITCODE) { throw "Clang object compilation failed: $level" }
        $executable = Join-Path $runDir ("dartforge.$level.exe")
        $arguments = @('aot', $source, $executable, '--link-object', $object, '--timings')
        if ($optimized) { $arguments += '--optimize' }
        $timingOutput = @(& $compiler @arguments)
        if ($LASTEXITCODE) { throw "DartForge FFI compilation failed: $level" }
        $timing = $timingOutput -join "`n" | ConvertFrom-Json
        $actual = @(& $executable)
        if ($LASTEXITCODE -or ($actual -join "`n") -cne $expected) { throw "DartForge FFI differs from SDK: $level" }
        $actual | Set-Content -Encoding utf8 (Join-Path $runDir ("dartforge.$level.stdout"))
        $results += [pscustomobject]@{ optimization=$level; status='pass'; output=$actual; timings=$timing }
    }
    $report = [ordered]@{
        schemaVersion=1; dartVersion=$version; generatedAt=(Get-Date -Format o)
        sourceSha256=(Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
        nativeSourceSha256=(Get-FileHash -LiteralPath $nativeSource -Algorithm SHA256).Hash
        oracle='SDK @Native unchanged; wrapper explicitly loads exported DLL before fixture.main'
        distinction='SDK resolves DLL symbols at runtime; DartForge links the matching C object explicitly'
        vm=$vm; aot=$aot; results=$results
    }
    $reportPath = Join-Path $runDir 'results.json'
    $report | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 $reportPath
    $results | Format-Table optimization,status -AutoSize
    Write-Host "Report: $reportPath"
} finally {
    $env:DARTFORGE_CLANG = $previousClang
    Pop-Location
}
