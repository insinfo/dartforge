# Serve um diretório de saída do `dartforge compile-js` por HTTP e, conforme o
# modo, abre-o no Edge headless para despejar o DOM (`-Headless`) ou conduzir um
# fluxo real de interações pelo CDP (`-Fluxo`).
#
# Uso:
#   pwsh scripts/servir.ps1 -Dir work/fe/out [-Porta 8765] [-Web C:/…/frontend/web]
#                           [-Headless] [-Fluxo] [-Json rel.json] [-Ddc] [-Visivel]
#
# `-Web` copia `index.html` (com `<script type="module" src="main.mjs">` no lugar
# do `main.dart.js`) e os assets do projeto para `-Dir`. Com `-Headless`, roda
# `msedge --headless=new --dump-dom` e mostra o DOM final e o coletor de erros
# que o index injeta (`window.__erros`). Com `-Fluxo`, delega ao
# `scripts/fluxo.mjs` (Node, sem dependências): ele serve o diretório com
# fallback de SPA — necessário porque o ngdart usa `routerProviders` com
# `<base href="/">` —, abre o Edge com `--remote-debugging-port` e executa a
# sequência de passos (carga, carrossel, login recusado, submeter "Entrar",
# rotas públicas, rota restrita), recolhendo console/onerror/unhandledrejection
# e as exceções do runtime em cada passo. `-Ddc` usa a saída do `dartdevc`
# (`import { main } from './main.js'`) em vez do `main.mjs`.
param(
  [string]$Dir = "work/fe/out",
  [int]$Porta = 8765,
  [string]$Web = "",
  [switch]$Headless,
  [switch]$Fluxo,
  [switch]$Ddc,
  [switch]$Visivel,
  [string]$Json = "",
  [string]$Rotulo = "DartForge",
  [int]$Espera = 8
)
$ErrorActionPreference = "Stop"
$Dir = (Resolve-Path $Dir).Path
if ($Fluxo) {
  $fluxoJs = Join-Path $PSScriptRoot "fluxo.mjs"
  $argumentos = @($fluxoJs, "--dir", $Dir, "--porta", "$Porta", "--rotulo", $Rotulo)
  if ($Web -ne "") { $argumentos += @("--web", $Web) }
  if ($Json -ne "") { $argumentos += @("--json", $Json) }
  if ($Ddc) { $argumentos += "--ddc" }
  if ($Visivel) { $argumentos += "--visivel" }
  & node @argumentos
  exit $LASTEXITCODE
}
if ($Web -ne "") {
  $html = Get-Content (Join-Path $Web "index.html") -Raw
  $coletor = @'
<script>
window.__erros = [];
function __reg(t) {
  window.__erros.push(t);
  var pre = document.getElementById("__erros");
  if (!pre) { pre = document.createElement("pre"); pre.id = "__erros"; (document.body || document.documentElement).appendChild(pre); }
  pre.textContent += t + "\n----\n";
}
window.onerror = function (m, s, l, c, e) { __reg(String(m) + " @" + s + ":" + l + (e && e.stack ? "\n" + e.stack : "")); };
window.addEventListener("unhandledrejection", function (ev) { var r = ev.reason; __reg("rejeição: " + String(r) + (r && r.stack ? "\n" + r.stack : "")); });
var __log = console.error; console.error = function () { __reg("console.error: " + Array.from(arguments).map(String).join(" ")); __log.apply(console, arguments); };
</script>
'@
  $html = $html -replace '<script[^>]*src="main\.dart\.js"[^>]*>\s*</script>', ''
  $html = $html -replace '</head>', ($coletor + "`n<script type=`"module`" src=`"main.mjs`"></script>`n</head>")
  Set-Content (Join-Path $Dir "index.html") $html -Encoding UTF8
  foreach ($a in @("assets", "favicon.ico", "manifest.json", "scrollbar.css")) {
    $src = Join-Path $Web $a
    if (Test-Path $src) { Copy-Item $src (Join-Path $Dir $a) -Recurse -Force }
  }
}
$servidor = Start-Process -FilePath "python" -ArgumentList @("-m", "http.server", "$Porta", "--bind", "127.0.0.1") -WorkingDirectory $Dir -PassThru -WindowStyle Hidden
try {
  Start-Sleep -Seconds 1
  $url = "http://127.0.0.1:$Porta/"
  if ($Headless) {
    $edge = "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
    if (-not (Test-Path $edge)) { $edge = "C:/Program Files/Microsoft/Edge/Application/msedge.exe" }
    # Perfil do Edge no D: (target/tmp-edge), nunca no %TEMP% do C:; apagado no finally.
    $perfil = Join-Path (Split-Path $PSScriptRoot -Parent) "target/tmp-edge"
    # `& msedge` perde o stdout no PowerShell; usa-se o Process diretamente.
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $edge
    $psi.Arguments = "--headless=new --disable-gpu --no-first-run --user-data-dir=`"$perfil`" --virtual-time-budget=$($Espera * 1000) --dump-dom $url"
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $p = [System.Diagnostics.Process]::Start($psi)
    $dom = $p.StandardOutput.ReadToEnd()
    $null = $p.StandardError.ReadToEnd()
    $p.WaitForExit()
    $dom
  } else {
    Write-Host "Servindo $Dir em $url (Ctrl+C para parar)"
    Wait-Process -Id $servidor.Id
  }
} finally {
  if (-not $servidor.HasExited) { Stop-Process -Id $servidor.Id -Force }
  if ($perfil -and (Test-Path $perfil)) { Remove-Item -Recurse -Force $perfil -ErrorAction SilentlyContinue }
}
