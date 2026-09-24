# Portão de TEMPO da regra de custo zero (PLANO.md, "geração de código e
# macros: rápidas, e custo zero para quem não usa"): com o motor de build e as
# macros compilados dentro, um projeto que não os usa compila e edita no mesmo
# tempo de antes. Compara dois conjuntos de binários — o do commit (-Atual) e o
# de referência, em geral o do main (-Base) — em rodadas ALTERNADAS, e reprova
# se a mediana do atual passar a da base mais a tolerância.
#
#   pwsh scripts/custo-zero.ps1 -Atual <dir> -Base <dir> [-Rodadas 5] [-Tolerancia 0.03]
#       mede (1) o corpus JS inteiro (corpus/js, um `dartforge compile-js` por
#       programa) e (2) a edição de corpo numa sessão sintética de ~300
#       bibliotecas (exemplo `medir` do crates/dev, média do platô).
#   pwsh scripts/custo-zero.ps1 -Atual <dir> -Base <dir> -Entrada <x.dart> -Packages <cfg> [-Alvo <arq>]
#       só a edição de corpo, num projeto real (ex.: new_sali/core; ver
#       scripts/medir-custo-zero.ps1).
#
# Cada <dir> tem `dartforge.exe` e `medir.exe` (o exemplo). Sai com código 1
# se alguma medida piorar além da tolerância; escreve a tabela em
# $env:GITHUB_STEP_SUMMARY quando existe.
param(
    [Parameter(Mandatory)][string]$Atual,
    [Parameter(Mandatory)][string]$Base,
    [int]$Rodadas = 5,
    [double]$Tolerancia = 0.03,
    [string]$Entrada,
    [string]$Packages,
    [string]$Alvo,
    [int]$Edicoes = 20
)
$ErrorActionPreference = 'Stop'
$raiz = Split-Path $PSScriptRoot -Parent
$tmp = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { Join-Path $raiz 'target/tmp-custo-zero' }
New-Item -ItemType Directory -Force $tmp | Out-Null

function Mediana([double[]]$v) {
    $s = $v | Sort-Object
    $n = $s.Count
    if ($n % 2) { return $s[[int][math]::Floor($n / 2)] }
    return ($s[$n / 2 - 1] + $s[$n / 2]) / 2
}

# (1) O corpus JS: tempo de relógio de compilar todos os programas.
function Corpus([string]$dir, [string]$saida) {
    $exe = Join-Path $dir 'dartforge.exe'
    $programas = Get-ChildItem (Join-Path $raiz 'corpus/js') -Filter *.dart | Sort-Object Name
    $t = [Diagnostics.Stopwatch]::StartNew()
    foreach ($p in $programas) {
        & $exe compile-js $p.FullName -o $saida *> $null
    }
    $t.Stop()
    return $t.Elapsed.TotalMilliseconds
}

# (2) Sessão residente: média de ms por edição de corpo (platô do `medir`).
function Sessao([string]$dir, [string]$entrada, [string]$cfg, [string]$alvo) {
    $exe = Join-Path $dir 'medir.exe'
    $env:DARTFORGE_DEV_SAIDA = Join-Path $tmp ('dev-' + (Split-Path $dir -Leaf))
    $a = @($entrada)
    if ($cfg) { $a += $cfg } else { $a += '' }
    $a += @($alvo, "$Edicoes")
    $saida = & $exe @a 2>&1 | Out-String
    # Tolerante à página de código do console (a saída do exemplo é UTF-8).
    # A média vem com duas casas (inteira em binários antigos, como o da base):
    # em ms inteiros, 1 ms a ~26 ms já passava da tolerância de 3%.
    if ($saida -notmatch 'm\S*dia (\d+(?:\.\d+)?) ms/edi') { throw "medir sem a linha de média:`n$saida" }
    return [double]::Parse($Matches[1], [Globalization.CultureInfo]::InvariantCulture)
}

# Projeto sintético: ~300 bibliotecas em cadeia, sem build_runner.
function Sintetico {
    $p = Join-Path $tmp 'sintetico'
    if (Test-Path (Join-Path $p 'main.dart')) { return $p }
    New-Item -ItemType Directory -Force (Join-Path $p 'lib'), (Join-Path $p '.dart_tool') | Out-Null
    Set-Content (Join-Path $p 'pubspec.yaml') "name: sintetico`nenvironment:`n  sdk: ^3.6.0`n" -Encoding utf8
    Set-Content (Join-Path $p '.dart_tool/package_config.json') '{"configVersion":2,"packages":[{"name":"sintetico","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"}]}' -Encoding utf8
    for ($i = 0; $i -lt 300; $i++) {
        $imp = if ($i -gt 0) { "import 'l$($i - 1).dart';`n" } else { '' }
        $uso = if ($i -gt 0) { "f$($i - 1)(x) + " } else { '' }
        Set-Content (Join-Path $p "lib/l$i.dart") "$imp`nclass C$i { final int v; C$i(this.v); int dobro() => v * 2; }`nint f$i(int x) => ${uso}C$i(x).dobro();`n" -Encoding utf8
    }
    Set-Content (Join-Path $p 'main.dart') "import 'package:sintetico/l299.dart';`nvoid main() { print(f299(1)); }`n" -Encoding utf8
    return $p
}

function Novas-Medidas {
    $m = [ordered]@{}
    if (-not $Entrada) {
        $m['corpus JS (ms)'] = @{ base = @(); atual = @() }
        $m['edição de corpo, 300 bibliotecas (ms)'] = @{ base = @(); atual = @() }
    }
    else {
        $m["edição de corpo, $(Split-Path $Entrada -Leaf) (ms)"] = @{ base = @(); atual = @() }
    }
    return $m
}
if (-not $Entrada) { $sint = Sintetico }

# Uma passada: $Rodadas rodadas, cada uma medindo base e atual em sequência
# (ordem alternada). O critério é a MEDIANA DAS RAZÕES PAREADAS atual/base de
# cada rodada: medidas feitas lado a lado dividem a mesma deriva do runner, que
# a comparação de medianas separadas não cancelava (medido: até 14% de ruído
# entre rodadas; o portão reprovava commits que não tocavam o compilador).
function Passada {
$medidas = Novas-Medidas
for ($r = 1; $r -le $Rodadas; $r++) {
    # Alterna a ordem: o que roda primeiro paga o disco frio.
    $ordem = if ($r % 2) { @('base', 'atual') } else { @('atual', 'base') }
    foreach ($qual in $ordem) {
        $dir = if ($qual -eq 'base') { $Base } else { $Atual }
        if (-not $Entrada) {
            $medidas['corpus JS (ms)'][$qual] += Corpus $dir (Join-Path $tmp "js-$qual")
            $medidas['edição de corpo, 300 bibliotecas (ms)'][$qual] += Sessao $dir (Join-Path $sint 'main.dart') (Join-Path $sint '.dart_tool/package_config.json') (Join-Path $sint 'lib/l150.dart')
        }
        else {
            $alvo = if ($Alvo) { $Alvo } else { $Entrada }
            $k = "edição de corpo, $(Split-Path $Entrada -Leaf) (ms)"
            $medidas[$k][$qual] += Sessao $dir $Entrada $Packages $alvo
        }
    }
    Write-Host "rodada $r de $Rodadas"
}
return $medidas
}

# Avalia uma passada: devolve (falhou, linhas da tabela).
function Avaliar($medidas, [string]$titulo) {
    $falhou = $false
    $md = @("### $titulo", '', "Rodadas: $Rodadas; tolerância: $([math]::Round($Tolerancia * 100, 1))% sobre a mediana das razões pareadas.", '', '| medida | base (mediana) | atual (mediana) | razão pareada (mediana) | razões por rodada |', '|---|---|---|---|---|')
    foreach ($k in $medidas.Keys) {
        $bs = $medidas[$k].base; $as = $medidas[$k].atual
        $razoes = @(for ($i = 0; $i -lt $bs.Count; $i++) { if ($bs[$i] -gt 0) { $as[$i] / $bs[$i] } else { 1.0 } })
        $razao = Mediana $razoes
        $ok = $razao -le (1 + $Tolerancia)
        if (-not $ok) { $falhou = $true }
        $md += "| $k | $([math]::Round((Mediana $bs), 1)) | $([math]::Round((Mediana $as), 1)) | $([math]::Round($razao, 3))$(if (-not $ok) { ' **piorou**' }) | $(($razoes | ForEach-Object { [math]::Round($_, 3) }) -join ', ') |"
    }
    return @{ falhou = $falhou; md = $md }
}

$primeira = Avaliar (Passada) 'Custo zero — o atual contra a base, razões pareadas'
$saida = $primeira.md
$falhou = $primeira.falhou
if ($falhou) {
    # Confirmação: uma regressão real persiste numa segunda passada; o azar do
    # runner, não. Só reprova se as duas passadas reprovarem.
    Write-Host 'primeira passada acima da tolerância; confirmando com uma segunda passada'
    $segunda = Avaliar (Passada) 'Confirmação (segunda passada)'
    $saida += @('') + $segunda.md
    $falhou = $segunda.falhou
}
$texto = $saida -join "`n"
Write-Host $texto
if ($env:GITHUB_STEP_SUMMARY) { Add-Content $env:GITHUB_STEP_SUMMARY $texto -Encoding utf8 }
if ($falhou) { Write-Host '::error::custo zero: o atual ficou mais lento que a base além da tolerância, em duas passadas'; exit 1 }
exit 0
