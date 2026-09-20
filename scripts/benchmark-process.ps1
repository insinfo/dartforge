param(
    [string]$DartExe = 'dart',
    [string]$NodeExe = 'node',
    [ValidateRange(3,100)][int]$Samples = 7,
    [ValidateRange(1,10000)][int]$Functions = 100
)
# Mede compilação por processos novos; não representa sessões incrementais de DDC.
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/env.ps1"
Push-Location (Split-Path $PSScriptRoot)
try {
    $dart = (Get-Command $DartExe -ErrorAction Stop).Source
    $node = (Get-Command $NodeExe -ErrorAction Stop).Source
    $version = (& $dart --version 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE) { throw 'Falha ao consultar versão do Dart' }
    if ($version -notmatch 'Dart SDK version: 3\.6\.2(?:\s|$)') { throw 'Benchmark exige Dart 3.6.2' }
    $snapshot = Join-Path (Split-Path $dart) 'snapshots/dartdevc.dart.snapshot'
    if (!(Test-Path -LiteralPath $snapshot)) { throw 'Snapshot DDC não encontrado' }
    cargo build --locked --release -p dartforge-cli
    if ($LASTEXITCODE) { throw 'Build falhou' }
    $metadata = cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE) { throw 'Falha ao consultar metadados Cargo' }
    $binary = if ($IsWindows) { 'dartforge.exe' } else { 'dartforge' }
    $compiler = Join-Path $metadata.target_directory "release/$binary"
    $directory = Join-Path $metadata.target_directory ('bench-process/' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Force $directory | Out-Null
    $inputFile = Join-Path $directory 'workload.dart'
    $builder = [Text.StringBuilder]::new()
    for ($i=0; $i -lt $Functions; $i++) {
        [void]$builder.AppendLine("int f$i(int n) { var sum=0; for(var i=0;i<n;i++){sum+=i;} return sum; }")
    }
    [void]$builder.AppendLine('void main() { var total=0;')
    for ($i=0; $i -lt $Functions; $i++) { [void]$builder.AppendLine("total += f$i(10);") }
    [void]$builder.AppendLine('print(total); }')
    [IO.File]::WriteAllText($inputFile, $builder.ToString())
    $timings = @{ dartforge = @(); dartforge_constants = @(); ddc = @(); dart2js_O2 = @() }
    $lastOutput = @{}
    $names = @('dartforge','dartforge_constants','ddc','dart2js_O2')
    for ($round=0; $round -le $Samples; $round++) {
        # Alterna a ordem para reduzir viés térmico/cache; primeira rodada é aquecimento.
        for ($position=0; $position -lt $names.Count; $position++) {
            $name = $names[($position+$round) % $names.Count]
            $extension = if ($name -in @('dartforge','dartforge_constants')) { '.mjs' } else { '.js' }
            $outputFile = Join-Path $directory "$name-$round$extension"
            $log = Join-Path $directory "$name-$round.log"
            $watch = [Diagnostics.Stopwatch]::StartNew()
            switch ($name) {
                'dartforge' { & $compiler compile $inputFile $outputFile *> $log }
                'dartforge_constants' { & $compiler compile $inputFile $outputFile --optimize *> $log }
                'ddc' { & $dart $snapshot --modules common -o $outputFile $inputFile *> $log }
                'dart2js_O2' { & $dart compile js -O2 -o $outputFile $inputFile *> $log }
            }
            $watch.Stop()
            if ($LASTEXITCODE) { throw "$name falhou; consulte $log" }
            if (!(Test-Path -LiteralPath $outputFile -PathType Leaf) -or (Get-Item -LiteralPath $outputFile).Length -eq 0) { throw "$name não produziu saída válida em $outputFile" }
            if ($round -gt 0) { $timings[$name] += $watch.Elapsed.TotalMilliseconds }
            $lastOutput[$name] = $outputFile
        }
    }
    $reference = @(& $node -e 'globalThis.self=globalThis; require(process.argv[1]);' $lastOutput.dart2js_O2)
    if ($LASTEXITCODE) { throw 'Execução dart2js falhou' }
    if (($reference -join "`n") -cne ([string](45 * $Functions))) { throw 'Resultado dart2js diverge da soma esperada do corpus sintético' }
    foreach ($variant in @('dartforge','dartforge_constants')) {
        $actual = @(& $node $lastOutput[$variant])
        if ($LASTEXITCODE) { throw "Execução $variant falhou" }
        if (($actual -join "`n") -cne ($reference -join "`n")) { throw "Saídas $variant/dart2js divergentes" }
        if (($actual -join "`n") -cne ([string](45 * $Functions))) { throw "Resultado $variant diverge da soma esperada do corpus sintético" }
    }
    $results = foreach ($name in $names) {
        $sorted = @($timings[$name] | Sort-Object)
        $middle = [int][Math]::Floor($sorted.Count / 2)
        $median = if ($sorted.Count % 2 -eq 0) { ($sorted[$middle-1] + $sorted[$middle]) / 2 } else { $sorted[$middle] }
        [ordered]@{compiler=$name; samples_ms=$timings[$name]; median_ms=$median; p95_ms=$sorted[[int][Math]::Ceiling($sorted.Count*0.95)-1]}
    }
    $report = [ordered]@{
        schema_version=1; kind='exploratory_fresh_process_warm_filesystem'; generated_at=[DateTimeOffset]::Now.ToString('o')
        dart_version=$version; node_version=(& $node --version); rust_version=(& rustc -Vv | Out-String).Trim(); git_revision=(& git rev-parse HEAD)
        git_status_porcelain=@(& git status --porcelain); rustflags=$env:RUSTFLAGS
        tools=[ordered]@{dart=$dart; node=$node; dartforge=$compiler; ddc_snapshot=$snapshot}
        dartforge_sha256=(Get-FileHash -LiteralPath $compiler -Algorithm SHA256).Hash
        percentile_method='nearest_rank'; order_policy='cyclic_rotation'; output_validation='last_sample_both_dartforge_variants_and_dart2js_plus_known_expected_sum'
        commands=[ordered]@{dartforge='dartforge compile INPUT OUTPUT'; dartforge_constants='dartforge compile INPUT OUTPUT --optimize'; ddc='dart dartdevc.dart.snapshot --modules common -o OUTPUT INPUT'; dart2js_O2='dart compile js -O2 -o OUTPUT INPUT'}
        os=[Runtime.InteropServices.RuntimeInformation]::OSDescription; cpu=$env:PROCESSOR_IDENTIFIER
        logical_processors=[Environment]::ProcessorCount; functions=$Functions; source_bytes=(Get-Item $inputFile).Length
        source_sha256=(Get-FileHash $inputFile -Algorithm SHA256).Hash; warmup_rounds=1; samples=$Samples
        dartforge_vs_dart2js_output_equal=$true; dartforge_constants_vs_dart2js_output_equal=$true; ddc_output_executed=$false
        warning='Não comprova superioridade: subconjunto sintético, DartForge sem otimização global/runtime completo; DDC sem worker incremental e saída não executada. Inclui inicialização, disco e invocação PowerShell; filesystem aquecido. Não mede RSS, recompilação incremental nem desempenho do JS.'
        results=$results
    }
    $report | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 (Join-Path $directory 'results.json')
    $results | ForEach-Object { [pscustomobject]$_ } | Format-Table compiler,median_ms,p95_ms
    Write-Output "Relatório: $directory/results.json"
} finally { Pop-Location }
