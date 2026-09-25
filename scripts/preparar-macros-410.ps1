$ErrorActionPreference = 'Stop'
$raiz = Split-Path -Parent $PSScriptRoot
$caso = Join-Path $raiz 'corpus/builders/macros_discovery'
$original = Join-Path $raiz 'corpus/macros/410_json_codable'
$trabalho = Join-Path $caso 'target/410-materializado'
$fonte = Join-Path $caso 'lib/modelos.dart'
$fonteForge = Join-Path $caso 'lib/modelos.forge.dart'
$augmentationForge = Join-Path $caso 'lib/modelos.forge.macro.dart'
$pedidoOriginal = Join-Path $raiz 'corpus/macros/411_pedido_independente'
$pedidoAugmentation = Join-Path $caso 'lib/pedido.macro.dart'
$utf8 = [Text.UTF8Encoding]::new($false)

New-Item -ItemType Directory -Force -Path $trabalho | Out-Null
$env:TEMP = $trabalho
$env:TMP = $trabalho
$env:PUB_CACHE = Join-Path $caso 'target/pub-cache'
[IO.File]::WriteAllBytes($fonteForge, [IO.File]::ReadAllBytes($fonte))

Push-Location $caso
try {
    dart pub get
    if ($LASTEXITCODE -ne 0) { throw 'pub get do builder 410 falhou' }
    dart run build_runner build --delete-conflicting-outputs
    if ($LASTEXITCODE -ne 0) { throw 'build_runner não materializou 410' }
    dart --enable-experiment=macros run check_model.dart
    if ($LASTEXITCODE -ne 0) { throw 'augmentation 410 divergiu do CFE' }
}
finally {
    Pop-Location
}

$esperado = [IO.File]::ReadAllText(
    (Join-Path $original 'esperado/modelos.augmentation.dart')).Replace(
    'package:caso_json/modelos.dart',
    'package:corpus_macros_discovery/modelos.forge.dart').Replace(
    "augment library 'package:corpus_macros_discovery/modelos.forge.dart';",
    "augment library 'modelos.forge.dart';").Replace([Environment]::NewLine, [string][char]10)
$obtido = [IO.File]::ReadAllText($augmentationForge).Replace([Environment]::NewLine, [string][char]10)
if ($obtido -cne $esperado) {
    throw 'augmentation da entrada Forge divergiu do CFE após normalizar URIs'
}
$pedidoEsperado = [IO.File]::ReadAllText(
    (Join-Path $pedidoOriginal 'esperado/pedido.augmentation.dart')).Replace(
    "augment library 'package:caso_pedido/pedido.dart';",
    "augment library 'pedido.dart';").Replace([Environment]::NewLine, [string][char]10)
$pedidoObtido = [IO.File]::ReadAllText($pedidoAugmentation).Replace([Environment]::NewLine, [string][char]10)
if ($pedidoObtido -cne $pedidoEsperado) {
    throw 'augmentation independente de Pedido divergiu do CFE'
}

$fonteTexto = [IO.File]::ReadAllText($fonteForge)
$importJson = "import 'package:json/json.dart';"
if (-not $fonteTexto.Contains($importJson)) {
    throw 'import do package:json ausente na entrada Forge'
}
$fonteTexto = $fonteTexto.Replace(
    $importJson, $importJson + [char]10 + "import augment 'modelos.forge.macro.dart';")
[IO.File]::WriteAllText($fonteForge, $fonteTexto, $utf8)
$main = [IO.File]::ReadAllText((Join-Path $original 'main.dart')).Replace(
    'package:caso_json/modelos.dart',
    'package:corpus_macros_discovery/modelos.forge.dart')
[IO.File]::WriteAllText((Join-Path $trabalho 'main.dart'), $main, $utf8)
Write-Host '410 preparado; augmentation independente de Pedido integral igual ao CFE'
