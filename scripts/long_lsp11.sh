#!/bin/sh
# Long LSP session on new_sali for one server (round 11 of docs/LSP.md).
# usage: [DRIVER=<editing_latency>] [JSON=1] [TAIL=MS] long_lsp11.sh <dartforge|dart> <rounds> <label> [out-dir] [server binary]
set -eu
which="$1"
rounds="$2"
label="$3"
out="${4:-/private/tmp/lsp11}"
here="$(cd "$(dirname "$0")/.." && pwd)"
server="${5:-$here/target/release/dartforge-lsp}"
driver="${DRIVER:-$here/target/release/examples/editing_latency}"
root=/Volumes/ssd_dock/MyProjectsDart/new_sali
front="$root/frontend/lib/src/modules/protocolo"
mkdir -p "$out"
case "$which" in
  dartforge) target="--server $server" ;;
  dart) target="--dart $HOME/tools/dart/3.6.2/dart-sdk/bin/dart" ;;
  *) echo "unknown server $which" >&2; exit 2 ;;
esac
# JSON=1: the last publication of every file in <out-dir>/<label>.json.
json=""
if [ -n "${JSON:-}" ]; then json="--json $out/$label.json"; fi
# TAIL=MS: the CPU spent in MS after each load settles.
if [ -n "${TAIL:-}" ]; then json="$json --tail $TAIL"; fi
echo "load before: $(uptime)"
# shellcheck disable=SC2086
"$driver" $target $json --label "$label" --root "$root" \
  --file "$front/components/processo_filtros/processo_filtros_component.dart" \
  --api "$front/services/documento_service.dart" \
  --long "$rounds" --generated 40 --csv "$out/$label.csv" \
  2> "$out/$label.stderr"
echo "load after: $(uptime)"
rm -rf "$root/frontend/lib/df_long_session"
