$ErrorActionPreference = 'Stop'
$raiz = Split-Path -Parent $PSScriptRoot
$caso = Join-Path $raiz 'corpus/macros/411_pedido_independente'
$trabalho = Join-Path $raiz 'target/macros-aot-sonda'
$binario = Join-Path $raiz 'target/release/dartforge.exe'
$dart = (Get-Command dart).Source
$entrada = Join-Path $caso 'main.dart'
$bootstrap = Join-Path $caso '.dart_tool/dartforge/macros/bootstrap.dart'
$pacotes = Join-Path $caso '.dart_tool/dartforge/macros/package_config.json'
$log = Join-Path $trabalho 'compile-native.txt'

New-Item -ItemType Directory -Force -Path $trabalho | Out-Null
$env:TEMP = $trabalho
$env:TMP = $trabalho
$env:PUB_CACHE = Join-Path $trabalho 'pub-cache'
Push-Location $caso
try {
    & $dart pub get --enforce-lockfile
    if ($LASTEXITCODE -ne 0) { throw 'pub get do caso 411 falhou' }
    & $binario macros $entrada --dart $dart --api (Join-Path $raiz 'pacotes/macros') `
        --versao-linguagem 3.6 --enable-experiment=macros > (Join-Path $trabalho 'macros.txt')
    if ($LASTEXITCODE -ne 0) { throw 'bootstrap do dfexec/1 não foi gerado' }
}
finally {
    Pop-Location
}
if (!(Test-Path -LiteralPath $bootstrap) -or !(Test-Path -LiteralPath $pacotes)) {
    throw 'bootstrap ou package_config do dfexec/1 ausente'
}

& $binario compile-native $bootstrap -o (Join-Path $trabalho 'executor.exe') `
    --packages $pacotes --sdk $env:DARTFORGE_SDK_LIB *> $log
$codigo = $LASTEXITCODE
$linhas = @(Get-Content -LiteralPath $log -Encoding utf8)
Write-Host "Sonda AOT dfexec/1: saída $codigo"
$linhas | Select-Object -First 80 | Write-Host
$resumo = @('### Sonda AOT do bootstrap de macros', '', "Código de saída: $codigo", '', '```text')
$resumo += $linhas | Select-Object -First 80
$resumo += '```'
Add-Content $env:GITHUB_STEP_SUMMARY ($resumo -join "`n") -Encoding utf8
# A sonda mede a lacuna do nativo; só a geração do bootstrap é pré-condição.
exit 0
