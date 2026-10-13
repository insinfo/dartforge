$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$case = Join-Path $root 'corpus/builders/macros_discovery'
Push-Location $case
try {
    dart pub get
    if ($LASTEXITCODE -ne 0) { throw 'pub get of the macros corpus failed' }
    dart run build_runner build --delete-conflicting-outputs
    if ($LASTEXITCODE -ne 0) { throw 'build_runner of the macros corpus failed' }
    $output = Join-Path $case 'lib/modelos.macro_uses.json'
    if (-not (Test-Path -LiteralPath $output)) { throw 'macro manifest missing' }
    $declarations = Join-Path $case 'lib/modelos.macro_declarations.json'
    if (-not (Test-Path -LiteralPath $declarations)) { throw 'declarations phase result missing' }
    $partial = Join-Path $case 'lib/modelos.macro_declarations.txt'
    if (-not (Test-Path -LiteralPath $partial)) { throw 'partial augmentation missing' }
    $definitionModel = Join-Path $case 'lib/modelos.macro_definitions_model.json'
    if (-not (Test-Path -LiteralPath $definitionModel)) { throw 'definitions model missing' }
    $definitionResult = Join-Path $case 'lib/modelos.macro_definitions.json'
    if (-not (Test-Path -LiteralPath $definitionResult)) { throw 'definitions result missing' }
    $complete = Join-Path $case 'lib/modelos.macro_complete.txt'
    if (-not (Test-Path -LiteralPath $complete)) { throw 'complete assembly missing' }
    $materialized = Join-Path $case 'lib/modelos.macro.dart'
    if (-not (Test-Path -LiteralPath $materialized)) { throw 'materialized augmentation missing' }
    $manifest = Get-Content -LiteralPath $output -Raw -Encoding utf8 | ConvertFrom-Json
    $names = @($manifest.applications | ForEach-Object { "$($_.target):$($_.class)" })
    $expected = @('Endereco:JsonCodable', 'Usuario:JsonCodable', 'SoSaida:JsonEncodable', 'SoEntrada:JsonDecodable')
    if ($manifest.version -ne 1 -or ($names -join '|') -ne ($expected -join '|')) {
        throw "different manifest: $(Get-Content -LiteralPath $output -Raw -Encoding utf8)"
    }
    dart --enable-experiment=macros run check_model.dart
    if ($LASTEXITCODE -ne 0) { throw 'the analyzer model differs from the CFE request' }
    Write-Host "macro discovery: $($names.Count)/$($expected.Count) applications resolved"
}
finally {
    Pop-Location
}
