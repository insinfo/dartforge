# Com e sem a poda do usuário (docs/JS-PRODUCAO.md §1.7 e §5): para cada
# programa, compila pelo `dartforge-jsprod` três vezes — sem poda do usuário,
# com poda e com poda em modo *stub* (o que o mundo diz morto denuncia a
# chamada com `DARTFORGE-PODADO:` e saída 97) — roda as três no Node e compara
# stdout e código de saída. Imprime tamanho e tempo de compilação de cada.
#
#   pwsh scripts/verificar-poda-js.ps1 [-Programas 01_print,57_nosuchmethod] [-Todos]
#   pwsh scripts/verificar-poda-js.ps1 -Entrada C:/MyDartProjects/new_sali/core/test/x_test.dart -Pacotes …/package_config.json
param(
  [string[]]$Programas = @(),
  [switch]$Todos,
  [string]$Entrada = "",
  [string]$Pacotes = "",
  [string]$Jsprod = ""
)
$ErrorActionPreference = "Stop"
$raiz = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if (-not $Jsprod) {
  $alvo = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $raiz "target" }
  $Jsprod = Join-Path $alvo "release/dartforge-jsprod.exe"
}
if (-not (Test-Path $Jsprod)) { throw "compile antes: cargo build --release -p dartforge-emit-js-producao" }
$trabalho = Join-Path $raiz "target/verificar-poda"
New-Item -ItemType Directory -Force $trabalho | Out-Null

$lista = @()
if ($Entrada) {
  $lista += [pscustomobject]@{ Nome = [IO.Path]::GetFileNameWithoutExtension($Entrada); Dart = $Entrada; Pacotes = $Pacotes }
} else {
  if ($Todos) {
    $Programas = Get-ChildItem (Join-Path $raiz "corpus/js") | ForEach-Object {
      if ($_.PSIsContainer) { if (Test-Path (Join-Path $_.FullName "main.dart")) { "$($_.Name)/main" } } elseif ($_.Extension -eq ".dart") { $_.BaseName }
    }
  }
  foreach ($p in ($Programas | ForEach-Object { $_ -split ',' } | Where-Object { $_ })) {
    $dart = Join-Path $raiz "corpus/js/$p.dart"
    $pc = Join-Path (Split-Path $dart) ".dart_tool/package_config.json"
    $lista += [pscustomobject]@{ Nome = ($p -replace '/', '_'); Dart = $dart; Pacotes = $(if (Test-Path $pc) { $pc } else { "" }) }
  }
}

function Compilar($item, $saida, $extra) {
  $argumentos = @($item.Dart, "-o", $saida) + $extra
  if ($item.Pacotes) { $argumentos += @("--packages", $item.Pacotes) }
  $t = [Diagnostics.Stopwatch]::StartNew()
  $log = & $Jsprod @argumentos 2>&1
  $ok = $LASTEXITCODE -eq 0
  [pscustomobject]@{ Ok = $ok; Segundos = $t.Elapsed.TotalSeconds; Log = ($log -join "`n"); Bytes = $(if (Test-Path $saida) { (Get-Item $saida).Length } else { 0 }) }
}

function Rodar($js) {
  Push-Location (Split-Path $js)
  $out = & node $js 2>$env:TEMP/verificar-poda-err.txt
  $codigo = $LASTEXITCODE
  Pop-Location
  [pscustomobject]@{ Stdout = ($out -join "`n"); Codigo = $codigo; Stderr = (Get-Content $env:TEMP/verificar-poda-err.txt -Raw) }
}

$falhas = 0
$linhas = @()
foreach ($item in $lista) {
  $dir = Join-Path $trabalho $item.Nome
  New-Item -ItemType Directory -Force $dir | Out-Null
  $sem = Compilar $item (Join-Path $dir "sem.js") @("--sem-poda-usuario")
  $com = Compilar $item (Join-Path $dir "com.js") @()
  $stub = Compilar $item (Join-Path $dir "stub.js") @("--verificar-stub")
  if (-not ($sem.Ok -and $com.Ok -and $stub.Ok)) {
    Write-Host "FALHA de compilação: $($item.Nome)`n$($com.Log)`n$($stub.Log)"
    $falhas++; continue
  }
  $rs = Rodar (Join-Path $dir "sem.js"); $rc = Rodar (Join-Path $dir "com.js"); $rt = Rodar (Join-Path $dir "stub.js")
  $igual = ($rs.Stdout -eq $rc.Stdout) -and ($rs.Codigo -eq $rc.Codigo) -and ($rs.Stdout -eq $rt.Stdout) -and ($rs.Codigo -eq $rt.Codigo)
  $podado = if ($rt.Stderr -match "DARTFORGE-PODADO: (.*)") { $Matches[1] } else { "" }
  if (-not $igual -or $podado) {
    $falhas++
    Write-Host "DIVERGE: $($item.Nome) (códigos $($rs.Codigo)/$($rc.Codigo)/$($rt.Codigo)) $podado"
    if ($rc.Stderr) { Write-Host ($rc.Stderr.Split("`n") | Select-Object -First 4 | Out-String) }
  }
  $mundo = ($com.Log -split "`n" | Where-Object { $_ -like "mundo:*" }) -join ""
  $linhas += "{0,-44} {1,9:N0} B -> {2,9:N0} B  {3,5:N2}s -> {4,5:N2}s  {5}" -f $item.Nome, $sem.Bytes, $com.Bytes, $sem.Segundos, $com.Segundos, $(if ($igual -and -not $podado) { "ok" } else { "DIVERGE" })
  if ($Entrada) { $linhas += $com.Log }
}
$linhas | ForEach-Object { Write-Host $_ }
Write-Host "$($lista.Count - $falhas)/$($lista.Count) iguais com e sem poda, sem stub executado"
exit $(if ($falhas -eq 0) { 0 } else { 1 })
