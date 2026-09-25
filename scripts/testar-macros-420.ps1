$ErrorActionPreference = 'Stop'
$raiz = Split-Path -Parent $PSScriptRoot
$caso = Join-Path $raiz 'corpus/macros/420_fase_tipos'
$trabalho = Join-Path $caso 'target/host-oraculo'
$binario = Join-Path $raiz 'target/release/dartforge.exe'
New-Item -ItemType Directory -Force -Path $trabalho | Out-Null
$env:TEMP = $trabalho
$env:TMP = $trabalho
$env:PUB_CACHE = Join-Path $trabalho 'pub-cache'

Push-Location $caso
try {
    & $binario macros (Join-Path $caso 'main.dart') --materializar --dart (Get-Command dart).Source `
        --api (Join-Path $raiz 'pacotes/macros') --versao-linguagem 3.6 --enable-experiment=macros
    if ($LASTEXITCODE -ne 0) { throw 'hospedeiro não materializou o 420' }
}
finally {
    Pop-Location
}
$esperado = [IO.File]::ReadAllText(
    (Join-Path $caso 'esperado/caso.augmentation.dart')).Replace(
    "augment library 'package:caso_fase_tipos/caso.dart';", "augment library 'caso.dart';").Replace(
    [Environment]::NewLine, [string][char]10)
$obtido = [IO.File]::ReadAllText((Join-Path $caso 'lib/caso.macro.dart')).Replace(
    [Environment]::NewLine, [string][char]10)
if ($obtido -cne $esperado) { throw '420: augmentation do hospedeiro divergiu byte a byte do CFE' }
Write-Host "420: augmentation de $($obtido.Length) caracteres igual ao CFE byte a byte"
