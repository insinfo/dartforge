# Prepara, serve e testa o `references/limitless_ui/example` nos dois lados:
# a saída oficial (`build_web_compilers`, release/dart2js) e a do
# `dartforge compile-js`. O critério de aceite é a suíte `ui_test/e2e`.
#
#   pwsh scripts/limitless-ui.ps1 -Preparar          # pub get + build_runner (+ release)
#   pwsh scripts/limitless-ui.ps1 -Montar            # compile-js + monta work/lui/out
#   pwsh scripts/limitless-ui.ps1 -Preparar -Producao # gerados, sem build oficial
#   pwsh scripts/limitless-ui.ps1 -Montar -Producao   # jsprod + work/lui/out-producao
#   pwsh scripts/limitless-ui.ps1 -Servir oficial    # serve example/build na 8081
#   pwsh scripts/limitless-ui.ps1 -Servir dartforge  # serve work/lui/out na 8081
#   pwsh scripts/limitless-ui.ps1 -E2e               # roda ui_test/e2e contra a 8081
#
# O servidor é o `tool/serve_example.dart` do próprio projeto nos dois casos,
# para que a única diferença entre as medições seja o JavaScript servido.
param(
  [switch]$Preparar,
  [switch]$Montar,
  [switch]$Producao,
  [string]$Servir = "",
  [switch]$E2e,
  [string]$Testes = "ui_test/e2e",
  [int]$Porta = 8081,
  [string]$Raiz = ""
)
$ErrorActionPreference = "Stop"
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if (-not $Raiz) { $Raiz = Join-Path $repo "references/limitless_ui" }
$Raiz = (Resolve-Path $Raiz).Path
$exemplo = Join-Path $Raiz "example"
$saida = Join-Path $repo $(if ($Producao) { "work/lui/out-producao" } else { "work/lui/out" })
$env:DARTFORGE_CACHE_DIR = Join-Path $repo "work/lui/cache"

if ($Preparar) {
  Push-Location $Raiz
  try { dart pub get; if ($LASTEXITCODE -ne 0) { throw "pub get falhou no pacote" } } finally { Pop-Location }
  Push-Location $exemplo
  try {
    dart pub get
    if ($LASTEXITCODE -ne 0) { throw "pub get falhou no exemplo" }
    dart run build_runner build --delete-conflicting-outputs
    if ($LASTEXITCODE -ne 0) { throw "geração falhou" }
    if (-not $Producao) {
      dart run build_runner build --release --output web:build --delete-conflicting-outputs
      if ($LASTEXITCODE -ne 0) { throw "build oficial falhou" }
    }
  } finally { Pop-Location }
}

if ($Montar) {
  New-Item -ItemType Directory -Force $saida | Out-Null
  if ($Producao) {
    & (Join-Path $repo "target/release/dartforge-jsprod.exe") `
      (Join-Path $exemplo "web/main.dart") -o (Join-Path $saida "main.dart.js") `
      --packages (Join-Path $exemplo ".dart_tool/package_config.json") `
      --dart-sdk-js (Join-Path $repo "runtime/ddc/dart_sdk.js")
    if ($LASTEXITCODE -ne 0) { throw "dartforge-jsprod falhou" }
  } else {
    & (Join-Path $repo "target/release/dartforge.exe") compile-js `
      (Join-Path $exemplo "web/main.dart") -o $saida `
      --packages (Join-Path $exemplo ".dart_tool/package_config.json")
    if ($LASTEXITCODE -ne 0) { throw "compile-js falhou" }
  }

  # `index.html`: injeta o coletor de erros (`window.__erros`), lido depois
  # pelo e2e. Desenvolvimento usa main.mjs; produção mantém main.dart.js.
  $html = Get-Content (Join-Path $exemplo "web/index.html") -Raw
  $coletor = @'
<script>
window.__erros = [];
window.onerror = function (m, s, l, c, e) { window.__erros.push(String(m) + " @" + s + ":" + l + (e && e.stack ? "\n" + e.stack : "")); };
window.addEventListener("unhandledrejection", function (ev) { var r = ev.reason; window.__erros.push("rejeicao: " + String(r) + (r && r.stack ? "\n" + r.stack : "")); });
var __log = console.error; console.error = function () { window.__erros.push("console.error: " + Array.from(arguments).map(String).join(" ")); __log.apply(console, arguments); };
</script>
'@
  if ($Producao) {
    # O bundle é um script clássico autoexecutável: mantém o mesmo carregamento
    # `defer` do build oficial e instala o coletor antes dele.
    $html = $html -replace '<script[^>]*src="main\.dart\.js"[^>]*>\s*</script>', ($coletor + "`n<script defer src=`"main.dart.js`"></script>")
  } else {
    $html = $html -replace '<script[^>]*src="main\.dart\.js"[^>]*>\s*</script>', ''
    $html = $html -replace '</head>', ($coletor + "`n<script type=`"module`" src=`"main.mjs`"></script>`n</head>")
  }
  Set-Content (Join-Path $saida "index.html") $html -Encoding UTF8

  # Estáticos do projeto e o `style.css` que o sass_builder gera do `style.scss`.
  foreach ($a in @("assets", "scrollbar.css", "themes", "favicon.ico")) {
    $src = Join-Path $exemplo "web/$a"
    if (Test-Path $src) { Copy-Item $src (Join-Path $saida $a) -Recurse -Force }
  }
  foreach ($css in @("style.css", "style.css.map")) {
    foreach ($c in @((Join-Path $exemplo "build/$css"), (Join-Path $exemplo ".dart_tool/build/generated/limitless_ui_example/web/$css"))) {
      if (Test-Path $c) { Copy-Item $c (Join-Path $saida $css) -Force; break }
    }
  }
}

if ($Servir -ne "") {
  $dir = if ($Servir -eq "oficial") { Join-Path $exemplo "build" } else { $saida }
  Push-Location $Raiz
  try { dart run tool/serve_example.dart --dir $dir --port $Porta --quiet } finally { Pop-Location }
}

if ($E2e) {
  $env:RUN_EXAMPLE_E2E = "true"
  $env:UI_EXAMPLE_BASE_URL = "http://127.0.0.1:$Porta"
  $env:UI_HEADLESS = "true"
  if (-not $env:PUPPETEER_EXECUTABLE_PATH) {
    foreach ($c in @("C:\Program Files\Google\Chrome\Application\chrome.exe", "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe")) {
      if (Test-Path $c) { $env:PUPPETEER_EXECUTABLE_PATH = $c; break }
    }
  }
  Push-Location $Raiz
  try {
    dart test $Testes -j 1 --reporter expanded
    if ($LASTEXITCODE -ne 0) { throw "suíte e2e falhou (código $LASTEXITCODE)" }
  } finally { Pop-Location }
}
