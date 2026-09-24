$ErrorActionPreference = 'Stop'
$raiz = Split-Path -Parent $PSScriptRoot
$caso = Join-Path $raiz 'corpus/builders/macros_discovery'
Push-Location $caso
try {
    dart pub get
    if ($LASTEXITCODE -ne 0) { throw 'pub get do corpus de macros falhou' }
    dart run build_runner build --delete-conflicting-outputs
    if ($LASTEXITCODE -ne 0) { throw 'build_runner do corpus de macros falhou' }
    $saida = Join-Path $caso 'lib/modelos.macro_uses.json'
    if (-not (Test-Path -LiteralPath $saida)) { throw 'manifesto de macros ausente' }
    $declaracoes = Join-Path $caso 'lib/modelos.macro_declarations.json'
    if (-not (Test-Path -LiteralPath $declaracoes)) { throw 'resultado da fase de declaracoes ausente' }
    $parcial = Join-Path $caso 'lib/modelos.macro_declarations.txt'
    if (-not (Test-Path -LiteralPath $parcial)) { throw 'augmentation parcial ausente' }
    $modeloDef = Join-Path $caso 'lib/modelos.macro_definitions_model.json'
    if (-not (Test-Path -LiteralPath $modeloDef)) { throw 'modelo de definicoes ausente' }
    $resultadoDef = Join-Path $caso 'lib/modelos.macro_definitions.json'
    if (-not (Test-Path -LiteralPath $resultadoDef)) { throw 'resultado de definicoes ausente' }
    $manifesto = Get-Content -LiteralPath $saida -Raw -Encoding utf8 | ConvertFrom-Json
    $nomes = @($manifesto.aplicacoes | ForEach-Object { "$($_.alvo):$($_.classe)" })
    $esperados = @('Endereco:JsonCodable', 'Usuario:JsonCodable', 'SoSaida:JsonEncodable', 'SoEntrada:JsonDecodable')
    if ($manifesto.versao -ne 1 -or ($nomes -join '|') -ne ($esperados -join '|')) {
        throw "manifesto diferente: $(Get-Content -LiteralPath $saida -Raw -Encoding utf8)"
    }
    dart --enable-experiment=macros run check_model.dart
    if ($LASTEXITCODE -ne 0) { throw 'modelo do analyzer diferente do pedido CFE' }
    Write-Host "descoberta de macros: $($nomes.Count)/$($esperados.Count) aplicações resolvidas"
}
finally {
    Pop-Location
}
