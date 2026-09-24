$ErrorActionPreference = 'Stop'
$raiz = Split-Path -Parent $PSScriptRoot
$caso = Join-Path $raiz 'corpus/macros/418_definicao_metodo'
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
    if ($LASTEXITCODE -ne 0) { throw 'hospedeiro não materializou o 418' }
}
finally {
    Pop-Location
}
$esperado = [IO.File]::ReadAllText(
    (Join-Path $caso 'esperado/calculo.augmentation.dart')).Replace(
    "augment library 'package:caso_definicao_metodo/calculo.dart';", "augment library 'calculo.dart';").Replace(
    [Environment]::NewLine, [string][char]10)
$obtido = [IO.File]::ReadAllText((Join-Path $caso 'lib/calculo.macro.dart')).Replace(
    [Environment]::NewLine, [string][char]10)
if ($obtido -cne $esperado) { throw '418: augmentation do hospedeiro divergiu byte a byte do CFE' }
Write-Host "418: augmentation de $($obtido.Length) caracteres igual ao CFE byte a byte"
