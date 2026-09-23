# Portão LOCAL da regra de custo zero, antes do merge: a edição de corpo no
# new_sali/core (sem builders; 227 ms no ESTADO §1.3) com o binário deste
# commit e com o do main, em rodadas alternadas (scripts/custo-zero.ps1).
# O projeto não está no CI.
#
#   pwsh scripts/medir-custo-zero.ps1 [-Rodadas 5] [-Tolerancia 0.03]
#
# O main é extraído com `git archive` em target/scratch-build/custo-zero e
# compilado num alvo próprio (a primeira vez leva uns minutos).
param(
    [string]$Entrada = 'C:/MyDartProjects/new_sali/core/test/arvore_processo_item_test.dart',
    [string]$Packages = 'C:/MyDartProjects/new_sali/core/.dart_tool/package_config.json',
    [int]$Rodadas = 5,
    [double]$Tolerancia = 0.03,
    [string]$Ref = 'main',
    # Reaproveita os binários da base de uma rodada anterior.
    [switch]$ReusarBase
)
$ErrorActionPreference = 'Stop'
$raiz = Split-Path $PSScriptRoot -Parent
if (-not $env:TEMP -or $env:TEMP -like 'C:*') { $env:TEMP = Join-Path $raiz 'target/tmp-build'; $env:TMP = $env:TEMP }
New-Item -ItemType Directory -Force $env:TEMP | Out-Null
$trabalho = Join-Path $raiz 'target/scratch-build/custo-zero'
New-Item -ItemType Directory -Force $trabalho | Out-Null

function Compilar([string]$fonte, [string]$alvo, [string]$destino) {
    Push-Location $fonte
    $env:CARGO_TARGET_DIR = $alvo
    # Dois comandos: `--example` restringe os alvos de todos os pacotes
    # pedidos, e o binário `dartforge` ficaria de fora.
    cargo build --release -p dartforge-cli
    $ok = $LASTEXITCODE -eq 0
    cargo build --release -p dartforge-dev --example medir
    $ok = $ok -and ($LASTEXITCODE -eq 0)
    Remove-Item Env:CARGO_TARGET_DIR
    Pop-Location
    if (-not $ok) { throw "build falhou em $fonte" }
    New-Item -ItemType Directory -Force $destino | Out-Null
    Copy-Item (Join-Path $alvo 'release/dartforge.exe'), (Join-Path $alvo 'release/examples/medir.exe') $destino -Force
}

$alvoAtual = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $raiz 'target' }
Compilar $raiz $alvoAtual (Join-Path $trabalho 'bin-atual')

$binBase = Join-Path $trabalho 'bin-base'
if (-not ($ReusarBase -and (Test-Path (Join-Path $binBase 'medir.exe')))) {
    $fonteBase = Join-Path $trabalho 'base-src'
    if (Test-Path $fonteBase) { Remove-Item -Recurse -Force $fonteBase }
    New-Item -ItemType Directory -Force $fonteBase | Out-Null
    $tar = Join-Path $trabalho 'base.tar'
    git -C $raiz archive --format=tar -o $tar $Ref
    if ($LASTEXITCODE -ne 0) { throw "git archive $Ref falhou" }
    tar -xf $tar -C $fonteBase
    Remove-Item $tar
    Compilar $fonteBase (Join-Path $trabalho 'base-target') $binBase
}

& (Join-Path $PSScriptRoot 'custo-zero.ps1') -Atual (Join-Path $trabalho 'bin-atual') -Base (Join-Path $trabalho 'bin-base') -Entrada $Entrada -Packages $Packages -Rodadas $Rodadas -Tolerancia $Tolerancia
exit $LASTEXITCODE
