# Regenera o oráculo do corpus do ngdart: roda o compilador oficial uma vez e
# guarda os `.template.dart` em corpus/ngdart/oraculo/.
#
# O builder do ngdart é `is_optional: true` — só roda quando alguém pede a
# saída —, por isso o corpus tem um `web/main.dart` que importa os templates e
# o `build_web_compilers` nas dependências de desenvolvimento. Esse main é
# gerado aqui a partir dos arquivos presentes: acrescentar um caso é só criar
# o `.dart` e o `.html` e rodar este script.
#
# O que o build_runner deixa em .dart_tool (81 MB) não é commitado; o oráculo,
# sim, porque é ele que os testes comparam byte a byte.
param([switch]$Limpar)

$ErrorActionPreference = 'Stop'
$raiz = (Resolve-Path (Join-Path $PSScriptRoot '..\corpus\ngdart')).Path
Push-Location $raiz
try {
    # 1. main.dart com um import por caso, para o builder opcional rodar.
    $casos = Get-ChildItem (Join-Path $raiz 'lib\src') -Filter *.dart |
        Where-Object { $_.Name -notlike '*.template.dart' } |
        Sort-Object Name
    $linhas = @(
        '// Consome os `.template.dart` do corpus. O builder do ngdart é',
        '// `is_optional: true`: sem alguém pedindo a saída, ele não roda.',
        '// Gerado por scripts/corpus-ngdart.ps1.'
    )
    $fabricas = @()
    foreach ($caso in $casos) {
        $nome = [IO.Path]::GetFileNameWithoutExtension($caso.Name)
        $partes = $nome.Split('_')
        $prefixo = $partes[0]
        $classe = $prefixo.Substring(0, 1).ToUpper() + $prefixo.Substring(1)
        foreach ($p in $partes[1..($partes.Length - 1)]) {
            $classe += $p.Substring(0, 1).ToUpper() + $p.Substring(1)
        }
        $linhas += "import 'package:corpus_ngdart/src/$nome.template.dart' as $prefixo;"
        $fabricas += "    $prefixo.${classe}NgFactory,"
    }
    $linhas += @('', 'void main() {', '  print([') + $fabricas + @('  ].length);', '}')
    $utf8 = New-Object System.Text.UTF8Encoding $false
    [IO.File]::WriteAllText((Join-Path $raiz 'web\main.dart'), ($linhas -join "`n") + "`n", $utf8)

    # 1b. a biblioteca que reexporta todos os casos, para o teste carregar o
    # pacote inteiro numa entrada só.
    $exports = @(
        '/// Biblioteca do corpus: existe para a carga alcançar todos os casos numa',
        '/// entrada só — é por ela que o teste monta o banco semântico.',
        'library corpus_ngdart;',
        ''
    )
    foreach ($caso in $casos) {
        $exports += "export 'src/$([IO.Path]::GetFileNameWithoutExtension($caso.Name)).dart';"
    }
    [IO.File]::WriteAllText((Join-Path $raiz 'lib\corpus_ngdart.dart'), ($exports -join "`n") + "`n", $utf8)

    # 2. o compilador oficial.
    dart pub get --offline
    dart run build_runner build --delete-conflicting-outputs

    # 3. o oráculo.
    $gerado = Join-Path $raiz '.dart_tool\build\generated\corpus_ngdart\lib\src'
    $destino = Join-Path $raiz 'oraculo'
    New-Item -ItemType Directory -Force $destino | Out-Null
    Copy-Item (Join-Path $gerado '*.template.dart') $destino -Force
    if (Test-Path (Join-Path $gerado '*.css.shim.dart')) {
        Copy-Item (Join-Path $gerado '*.css.shim.dart') $destino -Force
    }
    Write-Host "oráculo atualizado: $((Get-ChildItem $destino).Count) arquivos"
    if ($Limpar) { Remove-Item -Recurse -Force (Join-Path $raiz '.dart_tool') }
} finally {
    Pop-Location
}
