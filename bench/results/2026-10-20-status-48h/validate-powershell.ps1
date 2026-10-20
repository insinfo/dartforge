# Check the actual validator's parsed JIT invocation without starting a VM.
param([switch]$Baseline)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
$source = Get-Content -Raw (Join-Path $root 'scripts/validate-native-desktop.ps1')
if ($Baseline) {
    $source = (git -C $root show bf4129985ec7017633a4b7371b1c0e940eca5bc8:scripts/validate-native-desktop.ps1) -join "`n"
    if ($LASTEXITCODE -ne 0) { throw 'Could not read baseline validator' }
}
$tokens = $null
$parseErrors = $null
$tree = [System.Management.Automation.Language.Parser]::ParseInput($source, [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count -ne 0) { throw ($parseErrors | Out-String) }
$calls = $tree.FindAll({ param($node)
    $node -is [System.Management.Automation.Language.CommandAst] -and
    $node.GetCommandName() -eq 'Run-Case' -and $node.Extent.Text.Contains('name-jit-run')
}, $true)
if ($calls.Count -ne 1) { throw 'Expected one JIT invocation' }
$name = 'fixture'
$Compiler = 'compiler'
$entry = 'entry.dart'
$common = @('--sdk', 'sdk/lib', '--packages', 'package_config.json')
$programArgs = @('--sdk', 'application-owned', '--')
$marker = ''
function Run-Case($label, $program, $arguments, $marker) {
    $expected = @('run', '--sdk', 'sdk/lib', '--packages', 'package_config.json',
        'entry.dart', '--', '--sdk', 'application-owned', '--')
    if (($arguments -join "`n") -cne ($expected -join "`n")) {
        throw ('Wrong argument boundary: ' + ($arguments -join ' '))
    }
    [pscustomobject]@{ passed = $true; arguments = $arguments } | ConvertTo-Json -Depth 4
}
& ([scriptblock]::Create($calls[0].Extent.Text))
