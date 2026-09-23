# Regrava o oráculo do corpus de compatibilidade do motor de build
# (corpus/builders/): roda o build_runner OFICIAL em cada caso e guarda o que
# ele produziu em <caso>/oraculo/ — o plano (entrypoint/build.dart), as saídas
# `build_to: source`, as saídas de cache filtradas, a saída do programa e um
# manifesto com os sha256. O formato está em corpus/builders/README.md.
#
#   scripts/corpus-builders.ps1                 todos os casos
#   scripts/corpus-builders.ps1 -Caso freezed   só um
#   -Atualizar   `dart pub get` sem --enforce-lockfile (regrava pubspec.lock)
#   -Limpar      depois de gravar, apaga .dart_tool/build e as saídas source
#                da árvore do caso (o package_config.json fica)
#
# Um caso por vez, um processo dart por vez: a máquina é compartilhada.
param(
    [string]$Caso,
    [switch]$Atualizar,
    [switch]$Limpar
)

$ErrorActionPreference = 'Stop'
$corpus = (Resolve-Path (Join-Path $PSScriptRoot '..\corpus\builders')).Path
$utf8 = New-Object System.Text.UTF8Encoding $false

# Pacotes cujas versões entram no manifesto: a infraestrutura (sempre que
# estiver no lock) e os geradores/anotações (só quando o caso depende deles
# diretamente — json_annotation e built_value vêm transitivos do build_runner).
$pacotesInfra = @(
    'analyzer', 'build', 'build_config', 'build_resolvers', 'build_runner',
    'build_runner_core', 'dart_style', 'sass', 'source_gen'
)
$pacotesDiretos = @(
    'built_value', 'built_value_generator', 'drift', 'drift_dev', 'freezed',
    'freezed_annotation', 'json_annotation', 'json_serializable', 'mockito',
    'riverpod', 'riverpod_annotation', 'riverpod_generator', 'sass_builder'
)

# Saídas de cache que não são contrato: digests do resolvedor e tudo o que é
# de build_web_compilers/build_modules.
function Test-Excluido([string]$rel) {
    $nome = Split-Path $rel -Leaf
    return ($nome -like '*.transitive_digest') -or
        ($nome -like '*.module.library') -or
        ($nome -like '*.ddc.*') -or
        ($nome -like '*.meta_module.*') -or
        ($nome -like '*.module') -or
        ($nome -like '*.dart.js*') -or
        ($nome -like '*.dart.bootstrap.js') -or
        ($nome -like '*.digests') -or
        ($nome -like '*.ddc_merged_metadata')
}

function Get-Sha([string]$arquivo) {
    return (Get-FileHash -Algorithm SHA256 -LiteralPath $arquivo).Hash.ToLowerInvariant()
}

function ConvertTo-JsonTexto([string]$s) {
    $sb = New-Object System.Text.StringBuilder
    [void]$sb.Append('"')
    foreach ($ch in $s.ToCharArray()) {
        switch ($ch) {
            '"' { [void]$sb.Append('\"') }
            '\' { [void]$sb.Append('\\') }
            default {
                if ([int]$ch -lt 0x20) { [void]$sb.Append(('\u{0:x4}' -f [int]$ch)) }
                else { [void]$sb.Append($ch) }
            }
        }
    }
    [void]$sb.Append('"')
    return $sb.ToString()
}

# Versões do pubspec.lock: `  nome:` ... `    version: "x"`.
function Read-Lock([string]$lock) {
    $versoes = @{}
    $atual = $null
    foreach ($linha in Get-Content -LiteralPath $lock) {
        if ($linha -match '^  ([A-Za-z0-9_]+):\s*$') { $atual = $Matches[1] }
        elseif ($atual -and $linha -match '^    dependency: "?direct') { $versoes["direto:$atual"] = $true }
        elseif ($atual -and $linha -match '^    version: "?([^"]+)"?\s*$') {
            $versoes[$atual] = $Matches[1]
            $atual = $null
        }
    }
    return $versoes
}

# Arquivos da árvore do pacote que o build pode ter escrito (fora de
# .dart_tool, build/, oraculo/, edicoes/ e subpacotes com pubspec próprio).
function Get-Arvore([string]$raiz) {
    $ignorar = @('.dart_tool', 'build', 'oraculo', 'edicoes')
    $saida = @()
    $pilha = New-Object System.Collections.Stack
    $pilha.Push($raiz)
    while ($pilha.Count -gt 0) {
        $dir = $pilha.Pop()
        foreach ($f in Get-ChildItem -LiteralPath $dir -Force) {
            if ($f.PSIsContainer) {
                if ($dir -eq $raiz -and $ignorar -contains $f.Name) { continue }
                if (Test-Path -LiteralPath (Join-Path $f.FullName 'pubspec.yaml')) { continue }
                $pilha.Push($f.FullName)
            } else {
                $saida += $f
            }
        }
    }
    return $saida
}

function Get-Relativo([string]$base, [string]$caminho) {
    return $caminho.Substring($base.Length).TrimStart('\', '/').Replace('\', '/')
}

function Invoke-Dart([string[]]$argumentos) {
    & dart @argumentos
    if ($LASTEXITCODE -ne 0) { throw "dart $($argumentos -join ' ') falhou ($LASTEXITCODE)" }
}

function Update-Caso([string]$dir) {
    $dir = (Resolve-Path -LiteralPath $dir).Path
    if (-not $dir.StartsWith($corpus + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw "recusado: $dir não está sob $corpus"
    }
    $nomeCaso = Split-Path $dir -Leaf
    Write-Host "== $nomeCaso"
    Push-Location $dir
    try {
        $pubspec = Get-Content -LiteralPath (Join-Path $dir 'pubspec.yaml') -Raw
        if ($pubspec -notmatch '(?m)^name:\s*(\S+)') { throw "pubspec sem name: $dir" }
        $pacote = $Matches[1]
        $temBuild = $pubspec -match '(?m)^\s+build_runner:'

        if ($Atualizar) { Invoke-Dart @('pub', 'get') }
        else { Invoke-Dart @('pub', 'get', '--enforce-lockfile') }

        $oraculo = Join-Path $dir 'oraculo'
        foreach ($sub in @('source', 'cache')) {
            $p = Join-Path $oraculo $sub
            if (Test-Path -LiteralPath $p) { Remove-Item -LiteralPath $p -Recurse -Force }
        }
        foreach ($arq in @('plano.dart', 'saida.txt', 'manifesto.json')) {
            $p = Join-Path $oraculo $arq
            if (Test-Path -LiteralPath $p) { Remove-Item -LiteralPath $p -Force }
        }
        New-Item -ItemType Directory -Force $oraculo | Out-Null

        $saidas = @()
        $excluidos = 0
        if ($temBuild) {
            # Build limpo: sem grafo anterior, toda saída é escrita agora (as
            # que já existiam no disco são apagadas e reescritas) — é o que
            # permite achar as saídas source pela data de escrita, comparada
            # arquivo a arquivo com a de antes do build.
            $dirBuild = Join-Path $dir '.dart_tool\build'
            if (Test-Path -LiteralPath $dirBuild) { Remove-Item -LiteralPath $dirBuild -Recurse -Force }
            $antes = @{}
            foreach ($f in Get-Arvore $dir) { $antes[$f.FullName] = $f.LastWriteTimeUtc }

            Invoke-Dart @('run', 'build_runner', 'build', '--delete-conflicting-outputs')

            Copy-Item -LiteralPath (Join-Path $dirBuild 'entrypoint\build.dart') (Join-Path $oraculo 'plano.dart')

            # Saídas source: arquivos novos, ou reescritos durante o build.
            foreach ($f in Get-Arvore $dir) {
                $novo = -not $antes.ContainsKey($f.FullName)
                if ($novo -or $f.LastWriteTimeUtc -ne $antes[$f.FullName]) {
                    $rel = Get-Relativo $dir $f.FullName
                    if ($rel -eq 'pubspec.lock') { continue }
                    $alvo = Join-Path (Join-Path $oraculo 'source') $rel
                    New-Item -ItemType Directory -Force (Split-Path $alvo) | Out-Null
                    Copy-Item -LiteralPath $f.FullName $alvo
                    $saidas += [pscustomobject]@{ asset = "$pacote|$rel"; build_to = 'source'; sha256 = (Get-Sha $f.FullName); origem = $f.FullName }
                }
            }

            # Saídas cache: generated/<pkg>/<caminho>, filtradas.
            $gerado = Join-Path $dirBuild 'generated'
            if (Test-Path -LiteralPath $gerado) {
                foreach ($f in Get-ChildItem -LiteralPath $gerado -Recurse -File -Force) {
                    $rel = Get-Relativo $gerado $f.FullName
                    if (Test-Excluido $rel) { $excluidos++; continue }
                    $barra = $rel.IndexOf('/')
                    $pkg = $rel.Substring(0, $barra)
                    $caminho = $rel.Substring($barra + 1)
                    $alvo = Join-Path (Join-Path $oraculo 'cache') $rel
                    New-Item -ItemType Directory -Force (Split-Path $alvo) | Out-Null
                    Copy-Item -LiteralPath $f.FullName $alvo
                    $saidas += [pscustomobject]@{ asset = "$pkg|$caminho"; build_to = 'cache'; sha256 = (Get-Sha $f.FullName); origem = $null }
                }
            }
        }

        # O programa, com o build oficial aplicado.
        $executavel = Test-Path -LiteralPath (Join-Path $dir 'bin\main.dart')
        if ($executavel) {
            $psi = New-Object System.Diagnostics.ProcessStartInfo
            $psi.FileName = (Get-Command dart).Source
            $psi.Arguments = 'run bin/main.dart'
            $psi.WorkingDirectory = $dir
            $psi.UseShellExecute = $false
            $psi.RedirectStandardOutput = $true
            $psi.StandardOutputEncoding = $utf8
            $proc = [System.Diagnostics.Process]::Start($psi)
            $texto = $proc.StandardOutput.ReadToEnd()
            $proc.WaitForExit()
            if ($proc.ExitCode -ne 0) { throw "bin/main.dart saiu com $($proc.ExitCode)" }
            [IO.File]::WriteAllText((Join-Path $oraculo 'saida.txt'), $texto, $utf8)
        }

        # O manifesto.
        $versoes = Read-Lock (Join-Path $dir 'pubspec.lock')
        $linhas = @('{', '  "dart": "3.6.2",')
        $pacs = @(@($pacotesInfra + $pacotesDiretos) | Where-Object {
                $temBuild -and $versoes.ContainsKey($_) -and
                (($pacotesInfra -contains $_) -or $versoes.ContainsKey("direto:$_"))
            } | Sort-Object)
        if ($pacs.Count -eq 0) { $linhas += '  "pacotes": {},' }
        else {
            $linhas += '  "pacotes": {'
            for ($i = 0; $i -lt $pacs.Count; $i++) {
                $v = if ($i -lt $pacs.Count - 1) { ',' } else { '' }
                $linhas += "    $(ConvertTo-JsonTexto $pacs[$i]): $(ConvertTo-JsonTexto $versoes[$pacs[$i]])$v"
            }
            $linhas += '  },'
        }
        # Ordem ordinal (a de String.compareTo), não a da cultura.
        $lista = New-Object System.Collections.Generic.List[object]
        foreach ($s in $saidas) { $lista.Add($s) }
        $lista.Sort([Comparison[object]] { param($a, $b) [string]::CompareOrdinal($a.asset, $b.asset) })
        if ($lista.Count -eq 0) { $linhas += '  "saidas": [],' }
        else {
            $linhas += '  "saidas": ['
            for ($i = 0; $i -lt $lista.Count; $i++) {
                $s = $lista[$i]
                $v = if ($i -lt $lista.Count - 1) { ',' } else { '' }
                $linhas += "    { `"asset`": $(ConvertTo-JsonTexto $s.asset), `"build_to`": `"$($s.build_to)`", `"sha256`": `"$($s.sha256)`" }$v"
            }
            $linhas += '  ],'
        }
        $linhas += "  `"excluidos`": $excluidos,"
        $linhas += "  `"executavel`": $(if ($executavel) { 'true' } else { 'false' })"
        $linhas += '}'
        [IO.File]::WriteAllText((Join-Path $oraculo 'manifesto.json'), ($linhas -join "`n") + "`n", $utf8)

        $nSource = @($saidas | Where-Object { $_.build_to -eq 'source' }).Count
        $nCache = @($saidas | Where-Object { $_.build_to -eq 'cache' }).Count
        Write-Host "   $nSource source, $nCache cache, $excluidos excluídos, executável=$executavel"

        if ($Limpar -and $temBuild) {
            foreach ($s in $saidas) {
                if ($s.origem -and (Test-Path -LiteralPath $s.origem)) { Remove-Item -LiteralPath $s.origem -Force }
            }
            $dirBuild = Join-Path $dir '.dart_tool\build'
            if (Test-Path -LiteralPath $dirBuild) { Remove-Item -LiteralPath $dirBuild -Recurse -Force }
        }
    } finally {
        Pop-Location
    }
}

if ($Caso) {
    Update-Caso (Join-Path $corpus $Caso)
} else {
    foreach ($d in Get-ChildItem -LiteralPath $corpus -Directory | Sort-Object Name) {
        if (Test-Path -LiteralPath (Join-Path $d.FullName 'pubspec.yaml')) { Update-Caso $d.FullName }
    }
}
