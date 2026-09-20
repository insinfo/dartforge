param(
    [string]$DartExe = 'dart',
    [string]$NodeExe = 'node',
    [ValidateSet('O0','O1','O2','O3')]
    [string[]]$OptimizationLevels = @('O2'),
    [switch]$DartForgeOptimize,
    [switch]$MergeIdenticalFunctions,
    [switch]$AllowVersionMismatch
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/env.ps1"
$root = Split-Path $PSScriptRoot
Push-Location $root
try {
    $dartCommand = (Get-Command $DartExe -ErrorAction Stop).Source
    $nodeCommand = (Get-Command $NodeExe -ErrorAction Stop).Source
    $dartVersion = (& $dartCommand --version 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE) { throw 'Cannot read Dart version' }
    $matchesTarget = $dartVersion -match 'Dart SDK version: 3\.6\.2(?:\s|$)'
    if (!$matchesTarget -and !$AllowVersionMismatch) {
        throw "Target requires Dart 3.6.2; found $dartVersion. Use -DartExe to select it, or -AllowVersionMismatch for a clearly labeled exploratory run."
    }
    $nodeVersion = (& $nodeCommand --version | Out-String).Trim()
    cargo build --locked --release -p dartforge-cli
    if ($LASTEXITCODE) { throw 'DartForge build failed' }
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE) { throw 'Cargo metadata failed' }
    $binaryName = if ($IsWindows) { 'dartforge.exe' } else { 'dartforge' }
    $exe = Join-Path $metadata.target_directory ('release/' + $binaryName)
    $runDir = Join-Path $root ('target/conformance/' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Force $runDir | Out-Null
    $results = @()
    $cases = @(Get-ChildItem 'tests/conformance/cases/*.dart' | Sort-Object Name)
    if (Test-Path 'tests/conformance/modules') {
        $cases += @(Get-ChildItem 'tests/conformance/modules/*/main.dart' | Sort-Object FullName | ForEach-Object {
            [pscustomobject]@{ FullName=$_.FullName; BaseName=('module_' + $_.Directory.Name); Name=('modules/' + $_.Directory.Name + '/main.dart') }
        })
    }
    if (!$cases.Count) { throw 'No conformance cases found' }
    $levels = @($OptimizationLevels | Select-Object -Unique)
    if (!$levels.Count) { throw 'No optimization levels selected' }
    foreach ($case in $cases) {
        $actualJs = Join-Path $runDir ($case.BaseName + '.mjs')
        $actual = @()
        $actualError = $null
        try {
            $compilerArguments = @('compile', $case.FullName, $actualJs)
            if ($DartForgeOptimize) { $compilerArguments += '--optimize' }
            if ($MergeIdenticalFunctions) { $compilerArguments += '--merge-identical-functions' }
            & $exe @compilerArguments
            if ($LASTEXITCODE) { throw 'DartForge compilation failed' }
            $actual = @(& $nodeCommand $actualJs)
            if ($LASTEXITCODE) { throw 'DartForge JavaScript execution failed' }
        } catch { $actualError = $_.Exception.Message }
        foreach ($level in $levels) {
            $record = [ordered]@{ case=$case.Name; optimization=$level; status='fail'; actual=$actual; referenceOutput=$null; error=$null }
            try {
                if ($actualError) { throw $actualError }
                $referenceJs = Join-Path $runDir ($case.BaseName + '.' + $level + '.official.js')
                & $dartCommand compile js "-$level" -o $referenceJs $case.FullName
                if ($LASTEXITCODE) { throw 'Official Dart compilation failed' }
                $referenceOutput = @(& $nodeCommand -e 'globalThis.self = globalThis; require(process.argv[1]);' $referenceJs)
                if ($LASTEXITCODE) { throw 'Official JavaScript execution failed' }
                $record.referenceOutput = $referenceOutput
                if (($actual -join "`n") -cne ($referenceOutput -join "`n")) {
                    $record.status = 'divergence'
                    throw 'Outputs differ: investigate specification, backend semantics and compiler versions; do not assume either compiler is correct.'
                }
                $record.status = 'pass'
            } catch { $record.error = $_.Exception.Message }
            $results += [pscustomobject]$record
        }
    }
    $report = [ordered]@{
        target='3.6.2'; dartExecutable=$dartCommand; dartVersion=$dartVersion
        targetVersionMatched=$matchesTarget; nodeVersion=$nodeVersion
        dartforgeOptimization=if ($DartForgeOptimize) {'constants'} else {'none'}
        mergeIdenticalFunctions=[bool]$MergeIdenticalFunctions
        backend='dart2js vs DartForge release'; optimizationLevels=$levels
        generatedAt=(Get-Date -Format o); results=$results
    }
    $reportPath = Join-Path $runDir 'results.json'
    $report | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 $reportPath
    $results | Format-Table case,optimization,status,error -AutoSize
    Write-Host "Report: $reportPath"
    if (@($results | Where-Object status -ne 'pass').Count) { throw 'Conformance run contains failures or divergences; see report' }
    if (!$matchesTarget) { Write-Warning 'Exploratory comparison only: installed SDK differs from the Dart 3.6.2 target.' }
} finally { Pop-Location }
