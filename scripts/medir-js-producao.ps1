# Tamanho e tempo do perfil de produção contra o dart2js oficial.
#
#   pwsh scripts/medir-js-producao.ps1 [-Programas 01_print,50_classes_basico] [-Sdk C:/tools/dartsdk-3.6.2]
#
# Para cada programa de `corpus/js`, compila com o `dart compile js -O4` e com o
# `dartforge-jsprod`, e imprime os dois tamanhos e os dois tempos. A régua é o
# oficial; o alvo declarado em docs/JS-PRODUCAO.md §1.5 **não** é empatar com
# ele enquanto o runtime for o `dart_sdk.js` do DDC.
param(
  [string[]]$Programas = @(),
  [string]$Sdk = "E:/DartSDKs/3.6.2",
  [string]$Saida = ""
)
$ErrorActionPreference = "Stop"
$raiz = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$jsprod = Join-Path $raiz "target/release/dartforge-jsprod.exe"
if (-not (Test-Path $jsprod)) { throw "compile antes: cargo build --release -p dartforge-emit-js-producao" }
$trabalho = Join-Path $raiz "target/medir-js"
New-Item -ItemType Directory -Force $trabalho | Out-Null

if ($Programas.Count -eq 0) {
  # Uma amostra por tema, não o corpus inteiro: a medição custa um dart2js por programa.
  $Programas = @("01_print", "20_if_else_encadeado", "40_classes_basico", "60_list_basico",
                 "80_async_await_basico", "100_records_basico", "120_convert_json")
}

$linhas = @()
foreach ($p in $Programas) {
  $dart = Join-Path $raiz "corpus/js/$p.dart"
  if (-not (Test-Path $dart)) { Write-Host "pulado (não existe): $p"; continue }

  $oficial = Join-Path $trabalho "$p.dart2js.js"
  $t = [Diagnostics.Stopwatch]::StartNew()
  & "$Sdk/bin/dart.exe" compile js -O4 -o $oficial $dart 2>&1 | Out-Null
  $tOficial = $t.Elapsed.TotalSeconds

  $nosso = Join-Path $trabalho "$p.jsprod.js"
  $t = [Diagnostics.Stopwatch]::StartNew()
  & $jsprod $dart -o $nosso 2>&1 | Out-Null
  $tNosso = $t.Elapsed.TotalSeconds

  $a = (Get-Item $oficial).Length
  $b = (Get-Item $nosso).Length
  $linhas += [pscustomobject]@{
    programa   = $p
    dart2js_KB = [math]::Round($a / 1KB, 1)
    jsprod_KB  = [math]::Round($b / 1KB, 1)
    razao      = [math]::Round($b / $a, 1)
    dart2js_s  = [math]::Round($tOficial, 2)
    jsprod_s   = [math]::Round($tNosso, 2)
  }
}
$linhas | Format-Table -AutoSize
if ($Saida) { $linhas | ConvertTo-Json | Set-Content $Saida }
