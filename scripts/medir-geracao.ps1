# Latência de edição com o motor de geração ligado, no new_sali/frontend
# (plano do motor de build, §6.5; números no ESTADO.md). SEM build_runner: o
# apoio é o `.dart_tool/build/generated` que já existe lá.
#
#   pwsh scripts/medir-geracao.ps1 [-Projeto C:/MyDartProjects/new_sali/frontend] [-Repeticoes 3] [-SemMotor]
#
# Compila o exemplo `medir_geracao` (release) e roda: escolhe um componente
# cujo template sai do gerador nativo e um `.dart` sem Angular, aplica e
# reverte cada edição e imprime total, motor, nativo, recarga, escrita, ações,
# saídas, unidades e módulos por edição. `-SemMotor` mede a mesma sessão sem
# etapa (a linha de base), com os arquivos da última rodada com motor.
param(
    [string]$Projeto = 'C:/MyDartProjects/new_sali/frontend',
    [int]$Repeticoes = 3,
    [switch]$SemMotor
)
$ErrorActionPreference = 'Stop'
$raiz = Split-Path $PSScriptRoot -Parent
if (-not $env:TEMP -or $env:TEMP -like 'C:*') { $env:TEMP = Join-Path $raiz 'target/tmp-build'; $env:TMP = $env:TEMP }
New-Item -ItemType Directory -Force $env:TEMP | Out-Null
$alvo = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $raiz 'target' }
cargo build --release -p dartforge-dev --example medir_geracao
if ($LASTEXITCODE -ne 0) { throw 'build do exemplo falhou' }
if (-not $env:DARTFORGE_DART_SDK_JS) { $env:DARTFORGE_DART_SDK_JS = Join-Path $raiz 'runtime/ddc/dart_sdk.js' }
$env:DARTFORGE_AVISOS = '0'
$saida = Join-Path $raiz 'target/scratch-build/dev-geracao'
$a = @("$Projeto/web/main.dart", "$Projeto/.dart_tool/package_config.json", $saida, "$Repeticoes")
if ($SemMotor) { $a += 'sem-motor' }
& (Join-Path $alvo 'release/examples/medir_geracao.exe') @a 2>&1 | Where-Object { $_ -notmatch '^aviso: (Refer|Um valor|Nome|FutureOr)' }
