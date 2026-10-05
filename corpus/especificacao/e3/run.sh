#!/bin/bash
# uso: run.sh <subpasta>  -> saida compacta de todos os .dart da subpasta
cd /e/dftemp/analise/spec-r4/casos/e3
ANALYZER_STATE_LOCATION_OVERRIDE='E:\dftemp\analise\spec-r4\dartstate' /c/tools/dartsdk-3.6.2/bin/dart analyze --format=json "$1" > "$1/out.json" 2> "$1/err.txt"
python fmt.py "$1/out.json" "$1"
