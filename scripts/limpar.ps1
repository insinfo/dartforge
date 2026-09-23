# Vigia e limpa o lixo de build do DartForge.
#
# Por que existe: em um dia de trabalho com vários agentes o `target/` chegou a
# 38 GB (21 GB de `debug/deps` + 8 GB de compilação incremental) — tudo
# recriável, nada de código. O Cargo nunca apaga versões antigas sozinho.
#
#   pwsh scripts/limpar.ps1              # só relata o que ocupa espaço
#   pwsh scripts/limpar.ps1 -Limpar      # apaga o lixo seguro
#   pwsh scripts/limpar.ps1 -Limpar -Tudo  # também o release e o cache de oráculos
#
# O que é seguro apagar, e o custo de recriar:
#   target/debug            cache de build de desenvolvimento  (~10 min)
#   target/*/incremental    compilação incremental             (~5 min)
#   target-*                sobras de experimentos             (nada)
#   target/diferencial/nativo  executáveis e .ll do corpus nativo (nada; sempre)
#   target/native_cache/dartforge_runtime_*.lib  .lib antigas do runtime, fora
#                           as 2 mais recentes                (nada; sempre)
#   target/native_cache/obj cache de objeto do backend nativo (Clang de novo; só -Tudo;
#                           já se poda sozinho no teto de DARTFORGE_CACHE_OBJ_MB, 256 MB)
#   target/diferencial      cache dos oráculos do harness      (~10 min de `dart run`)
#   target/release          binários usados para medir         (~5 min)
param([switch]$Limpar, [switch]$Tudo, [int]$AlertaGB = 15)

$raiz = Split-Path -Parent $PSScriptRoot
function Tamanho($p) {
  if (-not (Test-Path $p)) { return 0 }
  ((Get-ChildItem $p -Recurse -File -Force -EA SilentlyContinue) | Measure-Object -Sum Length).Sum / 1GB
}

$alvos = @()
$alvos += [pscustomobject]@{ Caminho = "$raiz\target\debug"; Quando = 'sempre' }
$alvos += Get-ChildItem $raiz -Directory -Filter 'target-*' -EA SilentlyContinue |
  ForEach-Object { [pscustomobject]@{ Caminho = $_.FullName; Quando = 'sempre' } }
$alvos += Get-ChildItem "$raiz\.claude\worktrees" -Directory -EA SilentlyContinue |
  ForEach-Object { [pscustomobject]@{ Caminho = "$($_.FullName)\target"; Quando = 'sempre' } }
$alvos += [pscustomobject]@{ Caminho = "$raiz\target\release\incremental"; Quando = 'sempre' }
# Com -Tudo o diretório inteiro do harness sai logo abaixo; contar o `nativo`
# à parte somaria o mesmo espaço duas vezes.
if (-not $Tudo) {
  $alvos += [pscustomobject]@{ Caminho = "$raiz\target\diferencial\nativo"; Quando = 'sempre' }
}
$alvos += Get-ChildItem "$raiz\target\native_cache" -File -Filter 'dartforge_runtime_*.lib' -EA SilentlyContinue |
  Sort-Object LastWriteTime -Descending | Select-Object -Skip 2 |
  ForEach-Object { [pscustomobject]@{ Caminho = $_.FullName; Quando = 'sempre' } }
$alvos += [pscustomobject]@{ Caminho = "$raiz\target\native_cache\obj"; Quando = 'tudo' }
$alvos += [pscustomobject]@{ Caminho = "$raiz\target\diferencial"; Quando = 'tudo' }
$alvos += [pscustomobject]@{ Caminho = "$raiz\target\release"; Quando = 'tudo' }

$total = 0
foreach ($a in $alvos) {
  $gb = Tamanho $a.Caminho
  if ($gb -le 0) { continue }
  $marca = if ($a.Quando -eq 'tudo' -and -not $Tudo) { '(só com -Tudo)' } else { '' }
  "{0,8:N2} GB  {1} {2}" -f $gb, $a.Caminho.Replace("$raiz\", ''), $marca
  if ($a.Quando -eq 'sempre' -or $Tudo) {
    $total += $gb
    if ($Limpar) { Remove-Item $a.Caminho -Recurse -Force -EA SilentlyContinue }
  }
}

$livre = (Get-PSDrive ($raiz.Substring(0, 1))).Free / 1GB
if ($Limpar) {
  "--- apagados {0:N2} GB; livre agora: {1:N1} GB" -f $total, ((Get-PSDrive ($raiz.Substring(0, 1))).Free / 1GB)
} else {
  "--- {0:N2} GB de lixo; livre: {1:N1} GB (use -Limpar)" -f $total, $livre
  if ($livre -lt $AlertaGB) { Write-Warning "menos de $AlertaGB GB livres: rode com -Limpar" }
}
