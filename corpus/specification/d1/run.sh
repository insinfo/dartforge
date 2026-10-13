#!/bin/bash
cd /e/dftemp/analise/spec-r4/casos/d1
ANALYZER_STATE_LOCATION_OVERRIDE='E:\dftemp\analise\spec-r4\dartstate' /c/tools/dartsdk-3.6.2/bin/dart analyze --format=json ${1:-.} 2>&1 | python fmt.py
