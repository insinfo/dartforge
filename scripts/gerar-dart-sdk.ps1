# Gera runtime/ddc/dart_sdk.js (módulo ES6) a partir do ddc_platform.dill do SDK.
# É o dart:* oficial compilado pelo DDC; o DartForge emite no contrato de
# módulos do DDC e liga contra este arquivo. Uso:
#   pwsh scripts/gerar-dart-sdk.ps1 [-Sdk C:/tools/dartsdk-3.6.2]
param([string]$Sdk = "C:/tools/dartsdk-3.6.2")
$ErrorActionPreference = "Stop"
$saida = Join-Path $PSScriptRoot "../runtime/ddc"
New-Item -ItemType Directory -Force $saida | Out-Null
& "$Sdk/bin/dart.exe" "$Sdk/bin/snapshots/dartdevc.dart.snapshot" `
  --multi-root-scheme=org-dartlang-sdk --modules=es6 --module-name=dart_sdk `
  -o "$saida/dart_sdk.js" "$Sdk/lib/_internal/ddc_platform.dill"
Get-Item "$saida/dart_sdk.js" | Select-Object Name, Length
