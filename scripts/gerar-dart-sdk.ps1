# Gera runtime/ddc/dart_sdk.js (módulo ES6) a partir do ddc_platform.dill do SDK.
# É o dart:* oficial compilado pelo DDC; o DartForge emite no contrato de
# módulos do DDC e liga contra este arquivo. Uso:
#   pwsh scripts/gerar-dart-sdk.ps1 [-Sdk C:/tools/dartsdk-3.6.2] [-Saida runtime/ddc]
# O harness diferencial gera também o do SDK de oráculo 3.13 em
# runtime/ddc/<versão>/ (docs/VERSOES-LINGUAGEM.md §5): a coluna DDC de cada
# programa liga o dart_sdk.js do SDK que o compilou.
param([string]$Sdk = "E:/DartSDKs/3.6.2", [string]$Saida = "")
$ErrorActionPreference = "Stop"
$saida = if ($Saida) { $Saida } else { Join-Path $PSScriptRoot "../runtime/ddc" }
New-Item -ItemType Directory -Force $saida | Out-Null
& "$Sdk/bin/dart.exe" "$Sdk/bin/snapshots/dartdevc.dart.snapshot" `
  --multi-root-scheme=org-dartlang-sdk --modules=es6 --module-name=dart_sdk `
  -o "$saida/dart_sdk.js" "$Sdk/lib/_internal/ddc_platform.dill"
Get-Item "$saida/dart_sdk.js" | Select-Object Name, Length
