# Regenera o oráculo do corpus do ngdart: roda o compilador oficial uma vez e
# guarda os `.template.dart` em corpus/ngdart/oraculo/.
#
# O builder do ngdart é `is_optional: true` — só roda quando alguém pede a
# saída —, por isso o corpus tem um `web/main.dart` que importa os templates e
# o `build_web_compilers` nas dependências de desenvolvimento.
#
# O que o build_runner deixa em .dart_tool (81 MB) não é commitado; o oráculo,
# sim, porque é ele que os testes comparam byte a byte.
param([switch]$Limpar)

$ErrorActionPreference = 'Stop'
$raiz = Join-Path $PSScriptRoot '..\corpus\ngdart' | Resolve-Path
Push-Location $raiz
try {
    dart pub get --offline
    dart run build_runner build --delete-conflicting-outputs
    $gerado = Join-Path $raiz '.dart_tool\build\generated\corpus_ngdart\lib\src'
    $destino = Join-Path $raiz 'oraculo'
    New-Item -ItemType Directory -Force $destino | Out-Null
    Copy-Item (Join-Path $gerado '*.template.dart') $destino -Force
    Write-Host "oráculo atualizado: $((Get-ChildItem $destino).Count) arquivos"
    if ($Limpar) { Remove-Item -Recurse -Force (Join-Path $raiz '.dart_tool') }
} finally {
    Pop-Location
}
