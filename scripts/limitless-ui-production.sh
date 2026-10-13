#!/usr/bin/env bash
# e2e do `limitless_ui/example` com o bundle de produção (`dartforge-jsprod`),
# o equivalente de `scripts/limitless-ui.ps1 -Assemble -Production`, `-Servir` e
# `-E2e` para macOS e Linux.
#
#   scripts/limitless-ui-production.sh <raiz-do-limitless_ui> [saida] [-- args do jsprod]
#
# Pré-requisitos: `dart pub get` na raiz e no `example/`, e os gerados do
# `build_runner` em `example/.dart_tool/build/generated` (o `-Preparar
# -Producao` do script PowerShell). O Chrome é o de
# `PUPPETEER_EXECUTABLE_PATH`, senão o do `/Applications`.
# Variáveis: `DARTFORGE_JSPROD` (binário; padrão `target/release`), `PORTA`
# (8081). Sai com o código do `dart test`.
set -euo pipefail
repo="$(cd "$(dirname "$0")/.." && pwd)"
raiz="$(cd "$1" && pwd)"; shift
saida="${1:-$repo/work/lui/out-producao}"; [ $# -gt 0 ] && shift
[ "${1:-}" = "--" ] && shift
exemplo="$raiz/example"
jsprod="${DARTFORGE_JSPROD:-$repo/target/release/dartforge-jsprod}"
porta="${PORTA:-8081}"

mkdir -p "$saida"
"$jsprod" "$exemplo/web/main.dart" -o "$saida/main.dart.js" \
  --packages "$exemplo/.dart_tool/package_config.json" "$@"

# index.html com o coletor de erros (`window.__erros`, lido pelo e2e), antes
# do bundle, que é um script clássico com `defer` como no build oficial.
python3 - "$exemplo/web/index.html" "$saida/index.html" <<'EOF'
import re, sys
html = open(sys.argv[1], encoding="utf-8").read()
coletor = """<script>
window.__erros = [];
window.onerror = function (m, s, l, c, e) { window.__erros.push(String(m) + " @" + s + ":" + l + (e && e.stack ? "\\n" + e.stack : "")); };
window.addEventListener("unhandledrejection", function (ev) { var r = ev.reason; window.__erros.push("rejeicao: " + String(r) + (r && r.stack ? "\\n" + r.stack : "")); });
var __log = console.error; console.error = function () { window.__erros.push("console.error: " + Array.from(arguments).map(String).join(" ")); __log.apply(console, arguments); };
</script>
<script defer src="main.dart.js"></script>"""
html, n = re.subn(r'<script[^>]*src="main\.dart\.js"[^>]*>\s*</script>', lambda m: coletor, html)
assert n == 1, "index.html sem <script src=main.dart.js>"
open(sys.argv[2], "w", encoding="utf-8").write(html)
EOF
for a in assets scrollbar.css themes favicon.ico; do
  [ -e "$exemplo/web/$a" ] && cp -R "$exemplo/web/$a" "$saida/"
done
for css in style.css style.css.map; do
  for c in "$exemplo/build/$css" "$exemplo/.dart_tool/build/generated/limitless_ui_example/web/$css"; do
    if [ -f "$c" ]; then cp "$c" "$saida/$css"; break; fi
  done
done

(cd "$raiz" && exec dart run tool/serve_example.dart --dir "$saida" --port "$porta" --quiet) &
servidor=$!
trap 'kill $servidor 2>/dev/null || true' EXIT
for _ in $(seq 1 120); do
  curl -sf "http://127.0.0.1:$porta/" >/dev/null 2>&1 && break
  sleep 1
done

export RUN_EXAMPLE_E2E=true UI_EXAMPLE_BASE_URL="http://127.0.0.1:$porta" UI_HEADLESS=true
if [ -z "${PUPPETEER_EXECUTABLE_PATH:-}" ] && [ -x "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" ]; then
  export PUPPETEER_EXECUTABLE_PATH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
fi
cd "$raiz"
dart test ui_test/e2e -j 1 --reporter expanded
