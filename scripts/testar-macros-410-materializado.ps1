$ErrorActionPreference = 'Stop'
$raiz = Split-Path -Parent $PSScriptRoot
$caso = Join-Path $raiz 'corpus/builders/macros_discovery'
$original = Join-Path $raiz 'corpus/macros/410_json_codable'
$fonte = Join-Path $caso 'lib/modelos.dart'
$binario = Join-Path $raiz 'target/release/dartforge.exe'
$trabalho = Join-Path $caso 'target/410-materializado'
$saida = Join-Path $trabalho 'js'
$entrada = Join-Path $trabalho 'main.dart'
$utf8 = [Text.UTF8Encoding]::new($false)

if (-not (Test-Path -LiteralPath $binario)) {
    throw "binário DartForge ausente: $binario"
}
New-Item -ItemType Directory -Force -Path $trabalho | Out-Null
$env:TEMP = $trabalho
$env:TMP = $trabalho

Push-Location $original
try {
    if (-not (Test-Path '.dart_tool/package_config.json')) {
        dart pub get --enforce-lockfile
        if ($LASTEXITCODE -ne 0) { throw 'pub get do oráculo 410 falhou' }
    }
    $esperado = @(& dart --enable-experiment=macros run main.dart)
    if ($LASTEXITCODE -ne 0) { throw 'VM oficial não executou 410' }
}
finally {
    Pop-Location
}

$bytes = [IO.File]::ReadAllBytes($fonte)
Push-Location $caso
try {
    dart pub get
    if ($LASTEXITCODE -ne 0) { throw 'pub get do builder 410 falhou' }
    dart run build_runner build --delete-conflicting-outputs
    if ($LASTEXITCODE -ne 0) { throw 'build_runner não materializou 410' }
    dart --enable-experiment=macros run check_model.dart
    if ($LASTEXITCODE -ne 0) { throw 'augmentation 410 divergiu do CFE' }

    $fonteTexto = [Text.Encoding]::UTF8.GetString($bytes)
    $importJson = "import 'package:json/json.dart';"
    if (-not $fonteTexto.Contains($importJson)) {
        throw 'import do package:json não encontrado no fixture'
    }
    $importAugment = "import augment 'modelos.macro.dart';"
    $fonteComAugment = $fonteTexto.Replace($importJson, $importJson + [char]10 + $importAugment)
    [IO.File]::WriteAllText($fonte, $fonteComAugment, $utf8)

    $mainOriginal = [IO.File]::ReadAllText((Join-Path $original 'main.dart'))
    $mainBuilder = $mainOriginal.Replace(
        'package:caso_json/modelos.dart',
        'package:corpus_macros_discovery/modelos.dart')
    [IO.File]::WriteAllText($entrada, $mainBuilder, $utf8)
    & $binario compile-js $entrada -o $saida --enable-experiment=macros
    if ($LASTEXITCODE -ne 0) { throw 'compile-js não consumiu .macro.dart do builder' }
    $observado = @(& node (Join-Path $saida 'main.mjs'))
    if ($LASTEXITCODE -ne 0) { throw 'Node não executou 410 materializado' }
    if (($observado -join [char]10) -cne ($esperado -join [char]10)) {
        throw "410 divergiu da VM: esperado $($esperado -join ' | '); observado $($observado -join ' | ')"
    }
    Write-Host "410 materializado: $($observado.Count) linhas iguais à VM oficial"
}
finally {
    [IO.File]::WriteAllBytes($fonte, $bytes)
    Pop-Location
}
