param(
    [string]$DartExe = 'dart',
    [string]$ClangExe = '',
    [ValidateRange(1,100)][int]$Samples = 1
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/env.ps1"
$root = Split-Path $PSScriptRoot
$previousClang = $env:DARTFORGE_CLANG
Push-Location $root
try {
    $dartCommand = (Get-Command $DartExe -ErrorAction Stop).Source
    if (!$ClangExe) { $ClangExe = if ($env:DARTFORGE_CLANG) { $env:DARTFORGE_CLANG } else { 'clang' } }
    $clangCommand = (Get-Command $ClangExe -ErrorAction Stop).Source
    $version = (& $dartCommand --version 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE -or $version -notmatch 'Dart SDK version: 3\.6\.2(?:\s|$)') { throw "Requires Dart 3.6.2: $version" }
    $clangVersion = (& $clangCommand --version | Out-String).Trim()
    if ($LASTEXITCODE) { throw 'Cannot read Clang version' }
    $rustVersion = (& rustc -Vv | Out-String).Trim()
    $env:DARTFORGE_CLANG = $clangCommand
    cargo build --release --locked -p dartforge-cli
    if ($LASTEXITCODE) { throw 'DartForge build failed' }
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    $suffix = if ($IsWindows) { '.exe' } else { '' }
    $compiler = Join-Path $metadata.target_directory ('release/dartforge' + $suffix)
    $runDir = Join-Path $root ('target/native-conformance/' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Force $runDir | Out-Null
    $cases = @(Get-ChildItem tests/native/cases/*.dart | Sort-Object Name)
    $cases += @(Get-ChildItem tests/native/modules/*/main.dart | Sort-Object FullName | ForEach-Object {
        [pscustomobject]@{ FullName=$_.FullName; BaseName=('module_' + $_.Directory.Name); Name=('modules/' + $_.Directory.Name + '/main.dart') }
    })
    $results = @()
    foreach ($case in $cases) {
        $vmOutput = @(& $dartCommand $case.FullName)
        if ($LASTEXITCODE) { throw "Dart VM failed: $($case.Name)" }
        $officialTimes = @()
        for ($sample = 0; $sample -lt $Samples; $sample++) {
            $officialExe = Join-Path $runDir ($case.BaseName + '.official.' + $sample + $suffix)
            $timer = [Diagnostics.Stopwatch]::StartNew()
            & $dartCommand compile exe $case.FullName -o $officialExe
            if ($LASTEXITCODE) { throw "Dart AOT failed: $($case.Name)" }
            $officialTimes += $timer.Elapsed.TotalMilliseconds
            $officialOutput = @(& $officialExe)
            if ($LASTEXITCODE -or ($officialOutput -join "`n") -cne ($vmOutput -join "`n")) { throw 'Dart VM/AOT differ' }
        }
        foreach ($optimized in @($false, $true)) {
            $times = @()
            $phaseTimings = @()
            for ($sample = 0; $sample -lt $Samples; $sample++) {
                $nativeExe = Join-Path $runDir ($case.BaseName + '.forge.' + $optimized + '.' + $sample + $suffix)
                $arguments = @('aot', $case.FullName, $nativeExe, '--timings')
                if ($optimized) { $arguments += '--optimize' }
                $timer = [Diagnostics.Stopwatch]::StartNew()
                $timingJson = & $compiler @arguments
                if ($LASTEXITCODE) { throw "DartForge AOT failed: $($case.Name)" }
                $times += $timer.Elapsed.TotalMilliseconds
                $phase = $timingJson -join "`n" | ConvertFrom-Json
                $driverPhases = $phase.prepare_ns + $phase.clang_ns + $phase.rustc_link_ns + $phase.publish_ns
                if ($phase.schema_version -ne 1 -or $phase.backend -ne 'llvm' -or $phase.executable_bytes -ne (Get-Item -LiteralPath $nativeExe).Length -or $phase.driver_total_ns -lt $driverPhases -or $phase.total_ns -lt ($phase.frontend_ns + $phase.driver_total_ns)) {
                    throw 'Invalid AOT phase timing report'
                }
                $phaseTimings += $phase
                $actual = @(& $nativeExe)
                if ($LASTEXITCODE -or ($actual -join "`n") -cne ($vmOutput -join "`n")) { throw "Native divergence: $($case.Name), O2=$optimized" }
            }
            $results += [pscustomobject]@{ case=$case.Name; llvmOptimization=if ($optimized) {'O2'} else {'O0'}; status='pass'; actual=$actual; referenceOutput=$vmOutput; compileMs=$times; phaseTimings=$phaseTimings; officialCompileMs=$officialTimes }
        }
    }
    $report = [ordered]@{ gitRevision=(& git rev-parse HEAD | Out-String).Trim(); gitStatusPorcelain=(& git status --porcelain | Out-String).Trim(); target='3.6.2'; dartVersion=$version; clangVersion=$clangVersion; rustc=$rustVersion; samples=$Samples; generatedAt=(Get-Date -Format o); backend='LLVM object + Rust runtime/link'; note='Process timings include compiler startup, LLVM, runtime compilation and linking. Warm filesystem; no DDC/JS comparison; small subset, not production-performance evidence.'; results=$results }
    $reportPath = Join-Path $runDir 'results.json'
    $report | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 $reportPath
    $results | Format-Table case,llvmOptimization,status -AutoSize
    Write-Host "Report: $reportPath"
} finally {
    $env:DARTFORGE_CLANG = $previousClang
    Pop-Location
}
