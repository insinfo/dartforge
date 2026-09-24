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
    $manifesto = Get-Content -LiteralPath $saida -Raw -Encoding utf8 | ConvertFrom-Json
    $nomes = @($manifesto.aplicacoes | ForEach-Object { "$($_.alvo):$($_.classe)" })
    $esperados = @('Endereco:JsonCodable', 'Usuario:JsonCodable', 'SoSaida:JsonEncodable', 'SoEntrada:JsonDecodable')
    if ($manifesto.versao -ne 1 -or ($nomes -join '|') -ne ($esperados -join '|')) {
        throw "manifesto diferente: $($nomes -join ', ')"
    }
    Write-Host "descoberta de macros: $($nomes.Count)/$($esperados.Count) aplicações resolvidas"
}
finally {
    Pop-Location
}
