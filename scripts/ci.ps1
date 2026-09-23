# Atalho para o CI pesado (.github/workflows/pesado.yml): disparar, acompanhar
# e ler o placar sem rodar nada pesado nesta máquina. Ver ESTADO.md §3.
#
#   pwsh scripts/ci.ps1 -Frente nativo              # empurra HEAD para ci/nativo (roda `todos`)
#   pwsh scripts/ci.ps1 -Frente nativo -Acompanhar  # ... e acompanha até o fim
#   pwsh scripts/ci.ps1 -Suite nativo [-Ref ci/nativo] [-Fragmentos 8]
#                                                   # workflow_dispatch (exige pesado.yml no main)
#   pwsh scripts/ci.ps1 -Acompanhar [-Ref ci/nativo | -Run <id>]
#   pwsh scripts/ci.ps1 -Placar <run-id>            # placar de cada job (baixa os artefatos pequenos)
#   pwsh scripts/ci.ps1 -Listar                     # rodadas recentes de todas as branches ci/**, com placar
#
# Convenção: cada frente de trabalho tem a sua branch ci/<frente>. Rodadas de
# branches diferentes nunca se cancelam; uma rodada nova na MESMA branch
# cancela a anterior. As branches ci/** são descartáveis — o push é forçado.
param(
    [string]$Frente,
    [ValidateSet('todos', 'js', 'producao', 'nativo', 'determinismo', 'analise')][string]$Suite,
    [string]$Ref,
    [int]$Fragmentos = 0,
    [switch]$Acompanhar,
    [string]$Run,
    [string]$Placar,
    [switch]$Manter,
    [switch]$Listar
)
$ErrorActionPreference = 'Stop'
$workflow = 'pesado.yml'
$repo = gh repo view --json nameWithOwner -q .nameWithOwner
if (!$repo) { throw 'gh não autenticado ou fora do repositório' }

function Rodada-Da-Branch([string]$branch, [string]$sha) {
    # Espera o GitHub registrar a rodada (alguns segundos depois do push).
    for ($i = 0; $i -lt 30; $i++) {
        $runs = gh run list --workflow $workflow --branch $branch -L 5 --json databaseId,headSha,status | ConvertFrom-Json
        $r = if ($sha) { $runs | Where-Object headSha -eq $sha | Select-Object -First 1 } else { $runs | Select-Object -First 1 }
        if ($r) { return $r.databaseId }
        Start-Sleep -Seconds 4
    }
    throw "nenhuma rodada de $workflow em $branch" + $(if ($sha) { " para $sha" } else { '' })
}

# Placar publicado por cada job como anotação `::notice title=Placar <job>::…`.
function Placares([string]$id) {
    $jobs = (gh run view $id --json jobs | ConvertFrom-Json).jobs
    foreach ($j in $jobs) {
        $notas = gh api "repos/$repo/check-runs/$($j.databaseId)/annotations" 2>$null | ConvertFrom-Json
        foreach ($n in @($notas) | Where-Object { $_.title -like 'Placar*' }) {
            [pscustomobject]@{ job = $j.name; placar = $n.message }
        }
    }
}

$branchAlvo = $null
$shaAlvo = $null

if ($Frente) {
    if ($Frente -notmatch '^[A-Za-z0-9._-]+$') { throw "nome de frente inválido: $Frente" }
    $branchAlvo = "ci/$Frente"
    $shaAlvo = git rev-parse HEAD
    git push --force origin "HEAD:refs/heads/$branchAlvo"
    if ($LASTEXITCODE -ne 0) { throw 'git push falhou' }
    Write-Host ("empurrado $shaAlvo para $branchAlvo" + ' — pesado.yml roda a suíte "todos"')
}

if ($Suite) {
    $refDisparo = if ($Ref) { $Ref } elseif ($branchAlvo) { $branchAlvo } else { git branch --show-current }
    $a = @('workflow', 'run', $workflow, '--ref', $refDisparo, '-f', "suite=$Suite")
    if ($Fragmentos -gt 0) { $a += @('-f', "fragmentos=$Fragmentos") }
    gh @a
    if ($LASTEXITCODE -ne 0) { throw 'gh workflow run falhou (o pesado.yml já está no main? workflow_dispatch exige isso)' }
    $branchAlvo = $refDisparo
    $shaAlvo = $null
    Start-Sleep -Seconds 5
}

if ($Acompanhar) {
    $id = if ($Run) { $Run } else {
        $b = if ($branchAlvo) { $branchAlvo } elseif ($Ref) { $Ref } else { git branch --show-current }
        Rodada-Da-Branch $b $shaAlvo
    }
    Write-Host "acompanhando https://github.com/$repo/actions/runs/$id"
    gh run watch $id --compact --exit-status --interval 30
    $status = $LASTEXITCODE
    Placares $id | Format-Table -AutoSize | Out-String | Write-Host
    exit $status
}

if ($Placar) {
    gh run view $Placar
    # Fora do %TEMP%: o C: é pequeno e os downloads se acumulavam (10 GB em um
    # dia somados a outras sobras). Vai para target/ do repositório (no D:,
    # ignorado pelo git) e é apagado depois de impresso, salvo com -Manter.
    $dir = Join-Path $PSScriptRoot "..\target\ci-placar\$Placar"
    if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
    # Os artefatos relatorio-* e placar-* são texto pequeno; os binários não vêm.
    gh run download $Placar -D $dir -p 'relatorio-*' -p 'placar-*'
    Get-ChildItem $dir -Recurse -Filter placar.md | Sort-Object FullName | ForEach-Object {
        Write-Host "`n=== $($_.Directory.Name) ===" -ForegroundColor Cyan
        Get-Content $_ -Encoding utf8 | Write-Host
    }
    if ($Manter) {
        Write-Host "`nrelatórios completos em $dir"
    } else {
        Remove-Item -Recurse -Force $dir
        Write-Host "`n(relatórios apagados; use -Manter para guardá-los em target/ci-placar)"
    }
}

if ($Listar) {
    $runs = gh run list --workflow $workflow -L 60 --json databaseId,headBranch,status,conclusion,createdAt,event,headSha | ConvertFrom-Json
    # A mais recente de cada branch ci/** (e do main, que é o agendamento).
    $ultimas = $runs | Where-Object { $_.headBranch -like 'ci/*' -or $_.headBranch -eq 'main' } |
        Group-Object headBranch | ForEach-Object { $_.Group | Sort-Object createdAt -Descending | Select-Object -First 1 }
    foreach ($r in $ultimas | Sort-Object createdAt -Descending) {
        $estado = if ($r.status -eq 'completed') { $r.conclusion } else { $r.status }
        Write-Host ("{0,-22} {1,-12} {2}  {3}  https://github.com/{4}/actions/runs/{5}" -f $r.headBranch, $estado, $r.createdAt, $r.headSha.Substring(0, 8), $repo, $r.databaseId) -ForegroundColor Cyan
        foreach ($p in Placares $r.databaseId) { Write-Host ("    {0,-32} {1}" -f $p.job, $p.placar) }
    }
}

if (!$Frente -and !$Suite -and !$Acompanhar -and !$Placar -and !$Listar) {
    Get-Content $PSCommandPath | Select-Object -Skip 2 -First 14
}
