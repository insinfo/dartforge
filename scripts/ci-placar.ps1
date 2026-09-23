# Placar do harness diferencial em Markdown, para o $GITHUB_STEP_SUMMARY.
#
# Lê um ou mais relatórios de `dartforge-diferencial` (o stdout dele) e escreve
# na saída: o placar, o tempo e o agrupamento de falhas. Com vários relatórios
# (os fragmentos do corpus nativo), SOMA os placares e FUNDE os grupos de
# falha pela chave — o mesmo agrupamento que o harness daria rodando o corpus
# inteiro numa máquina só.
#
#   pwsh scripts/ci-placar.ps1 -Relatorio relatorio.txt -Titulo 'JS desenvolvimento'
#   pwsh scripts/ci-placar.ps1 -Relatorio (ls rel -Recurse -Filter relatorio.txt) -Titulo 'Nativo' -Esperados 8
#   pwsh scripts/ci-placar.ps1 -Relatorio relatorio.txt -Linha     # só o placar, numa linha
param(
    [Parameter(Mandatory)][string[]]$Relatorio,
    [string]$Titulo = 'Placar',
    # Quantos relatórios deviam existir (fragmentos); a falta é dita no topo.
    [int]$Esperados = 0,
    # Uma linha só (para a anotação `::notice` do job).
    [switch]$Linha
)
$ErrorActionPreference = 'Stop'

# Placar por executor: rótulo -> [ok, total]. O DDC×VM tem formato próprio.
$placar = [ordered]@{}
$ddc = $null
# Grupos de falha: seção -> (chave -> lista de programas).
$grupos = [ordered]@{}
$programas = 0
$segundos = @()
$dartQuebrado = @()
$pendentes = $null
# Seção do `--jit-aot` (acordo JIT × AOT e tempos pós-IR), copiada como está.
$jitAot = [System.Collections.Generic.List[string]]::new()
$lidos = 0

foreach ($arq in $Relatorio) {
    if (!(Test-Path $arq)) { continue }
    $lidos++
    $linhas = Get-Content -LiteralPath $arq -Encoding utf8
    $secao = $null
    for ($i = 0; $i -lt $linhas.Count; $i++) {
        $l = $linhas[$i]
        if ($l -match '^(JIT × AOT: |JIT≠AOT |Tempos pós-IR|  (JIT|AOT) |  razão |  tempo esgotado nos dois)') { $jitAot.Add($l) }
        if ($l -match '^(DartForge [^:]+):\s+(\d+)/(\d+) ok') {
            $r = $Matches[1]
            if (!$placar.Contains($r)) { $placar[$r] = @(0, 0) }
            $placar[$r] = @(($placar[$r][0] + [int]$Matches[2]), ($placar[$r][1] + [int]$Matches[3]))
            $secao = $null
        } elseif ($l -match '^Pendentes \(PENDENTES\): (\d+) ainda falham, (\d+) passaram') {
            $pendentes = @([int]$Matches[1], [int]$Matches[2])
        } elseif ($l -match '^DDC×VM: (\d+)/(\d+) batem \(sem contar (\d+)') {
            if (!$ddc) { $ddc = @(0, 0, 0) }
            $ddc = @(($ddc[0] + [int]$Matches[1]), ($ddc[1] + [int]$Matches[2]), ($ddc[2] + [int]$Matches[3]))
        } elseif ($l -match '^\((\d+) programas em ([\d.,]+) s\)') {
            $programas += [int]$Matches[1]
            $segundos += [double]($Matches[2] -replace ',', '.')
        } elseif ($l -match '^DART!\s+(\S+)') {
            $dartQuebrado += $Matches[1]
        } elseif ($l -match '^Falhas por primeira linha do stderr (.+?) \(prioridade') {
            $secao = $Matches[1]
            if (!$grupos.Contains($secao)) { $grupos[$secao] = [ordered]@{} }
        } elseif ($secao -and $l -match '^\s{2}\s*\d+  (.*)$' -and ($i + 1) -lt $linhas.Count -and $linhas[$i + 1] -match '^\s{8}(\S.*)$') {
            $chave = $l -replace '^\s*\d+  ', ''
            $nomes = $linhas[$i + 1].Trim() -split ', '
            if (!$grupos[$secao].Contains($chave)) { $grupos[$secao][$chave] = [System.Collections.Generic.List[string]]::new() }
            $grupos[$secao][$chave].AddRange([string[]]$nomes)
            $i++
        }
    }
}

# Ordem do corpus: prefixo numérico, depois o nome (como `corpus::listar`).
function Ordem([string]$n) {
    $num = if ($n -match '^(\d+)') { [int]$Matches[1] } else { [int]::MaxValue }
    return '{0:D10}{1}' -f $num, $n
}

$resumo = @($placar.Keys | ForEach-Object { "$_ $($placar[$_][0])/$($placar[$_][1])" })
if ($Linha) {
    $txt = if ($resumo) { $resumo -join '; ' } else { 'sem placar no relatório' }
    if ($Esperados -gt 0 -and $lidos -lt $Esperados) { $txt += " (só $lidos de $Esperados fragmentos)" }
    Write-Output $txt
    return
}

$out = [System.Collections.Generic.List[string]]::new()
$out.Add("### $Titulo")
$out.Add('')
if ($Esperados -gt 0 -and $lidos -lt $Esperados) {
    $out.Add("> **Atenção: só $lidos de $Esperados relatórios chegaram** — o placar abaixo é parcial; veja os jobs que falharam.")
    $out.Add('')
}
if ($lidos -eq 0) {
    $out.Add('Nenhum relatório encontrado — o harness não chegou a escrever o relatório.')
} else {
    $out.Add('| executor | placar |')
    $out.Add('| --- | ---: |')
    foreach ($r in $placar.Keys) { $out.Add("| $r | **$($placar[$r][0])/$($placar[$r][1])** |") }
    if ($pendentes) { $out.Add("| pendentes (PENDENTES) | $($pendentes[0]) ainda falham, $($pendentes[1]) passaram |") }
    if ($ddc) { $out.Add("| DDC×VM (oráculos) | $($ddc[0])/$($ddc[1] - $ddc[2]) (+$($ddc[2]) com divergência declarada) |") }
    $out.Add('')
    if ($segundos.Count -eq 1) {
        $out.Add("$programas programas em $([math]::Round($segundos[0])) s.")
    } elseif ($segundos.Count -gt 1) {
        $max = ($segundos | Measure-Object -Maximum).Maximum
        $soma = ($segundos | Measure-Object -Sum).Sum
        $out.Add("$programas programas em $($segundos.Count) relatórios; o mais lento levou $([math]::Round($max)) s, soma $([math]::Round($soma)) s.")
    }
    if ($dartQuebrado) {
        $out.Add('')
        # Esperado nos programas só-web (`// diverge-ddc:`), cuja referência é o DDC.
        $out.Add('`dart run` saiu com código ≠ 0 em ' + $dartQuebrado.Count + ' (linhas `DART!` do relatório): ' + (($dartQuebrado | Sort-Object { Ordem $_ }) -join ', '))
    }
    if ($jitAot.Count -gt 0) {
        $out.Add('')
        $out.Add('```text')
        foreach ($l in $jitAot) { $out.Add($l) }
        $out.Add('```')
    }
    foreach ($secao in $grupos.Keys) {
        $g = $grupos[$secao]
        $total = ($g.Values | ForEach-Object { $_.Count } | Measure-Object -Sum).Sum
        $out.Add('')
        $out.Add("<details><summary>Falhas $secao por primeira linha do stderr: $total programas em $($g.Count) grupos</summary>")
        $out.Add('')
        $out.Add('```text')
        foreach ($chave in ($g.Keys | Sort-Object @{ Expression = { $g[$_].Count }; Descending = $true }, @{ Expression = { $_ } })) {
            $out.Add(('{0,6}  {1}' -f $g[$chave].Count, $chave))
            $out.Add('        ' + (($g[$chave] | Sort-Object { Ordem $_ }) -join ', '))
        }
        $out.Add('```')
        $out.Add('')
        $out.Add('</details>')
    }
}
$out -join "`n"
