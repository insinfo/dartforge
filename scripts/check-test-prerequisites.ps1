$ErrorActionPreference = 'Stop'

foreach ($tool in @('cargo', 'dart', 'node')) {
    if (!(Get-Command $tool -CommandType Application -ErrorAction SilentlyContinue)) {
        throw "Required test tool '$tool' was not found on PATH. Configure the prerequisites in CONTRIBUTING.md before running checks."
    }
}

# The differential harness selects SDK roots independently of Dart on PATH
# and DARTFORGE_SDK_LIB. Check them before starting expensive workspace builds.
foreach ($variable in @('DARTFORGE_DART_SDK', 'DARTFORGE_DART_SDK_3_13')) {
    $sdkRoot = [Environment]::GetEnvironmentVariable($variable)
    if (!$sdkRoot) {
        throw "Set $variable to a host-compatible oracle SDK root (containing bin and lib). See CONTRIBUTING.md."
    }
    $executable = if ($IsWindows) { 'dart.exe' } else { 'dart' }
    foreach ($relative in @('version', "bin/$executable", 'bin/snapshots/dartdevc.dart.snapshot', 'lib/libraries.json')) {
        if (!(Test-Path -LiteralPath (Join-Path $sdkRoot $relative) -PathType Leaf)) {
            throw "$variable is missing '$relative': $sdkRoot. Select a complete SDK for this host; see CONTRIBUTING.md."
        }
    }
    $sdkVersion = (Get-Content -LiteralPath (Join-Path $sdkRoot 'version') -Raw).Trim()
    $parsedVersion = $null
    if (($variable -eq 'DARTFORGE_DART_SDK' -and $sdkVersion -ne '3.6.2') -or
        ($variable -eq 'DARTFORGE_DART_SDK_3_13' -and
            (![Version]::TryParse(($sdkVersion -split '-')[0], [ref]$parsedVersion) -or $parsedVersion -lt [Version]'3.13.0'))) {
        throw "$variable has unsupported oracle version '$sdkVersion'. Use 3.6.2 for the floor and a compatible 3.13+ SDK for modern cases."
    }
    $versionOutput = & (Join-Path $sdkRoot "bin/$executable") --version 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0 -or !$versionOutput.Contains("Dart SDK version: $sdkVersion ")) {
        throw "$variable cannot execute the SDK version declared by its version file: $sdkRoot."
    }
}
