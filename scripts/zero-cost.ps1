# TIME gate of the zero-cost rule (PLANO.md, "geração de código e macros:
# rápidas, e custo zero para quem não usa"): with the build engine and the
# macros compiled in, a project that does not use them compiles and edits in
# the same time as before. Compares two sets of binaries — the commit's
# (-Current) and the reference, usually main's (-Base) — in ALTERNATING
# rounds, and fails if the current median exceeds the base's plus the tolerance.
#
#   pwsh scripts/zero-cost.ps1 -Current <dir> -Base <dir> [-Rounds 5] [-Tolerance 0.10]
#       measures (1) the whole JS corpus (corpus/js, one `dartforge compile-js` per
#       program) and (2) the body edit in a synthetic session of ~300
#       libraries (crates/dev's `measure` example, plateau mean).
#   pwsh scripts/zero-cost.ps1 -Current <dir> -Base <dir> -Entry <x.dart> -Packages <cfg> [-Target <file>]
#       only the body edit, in a real project (e.g. new_sali/core; see
#       scripts/measure-zero-cost.ps1).
#
# Each <dir> has `dartforge.exe` and `measure.exe` (the example). Exits with
# code 1 if any measurement gets worse beyond the tolerance; writes the table
# to $env:GITHUB_STEP_SUMMARY when it exists.
param(
    [Parameter(Mandatory)][string]$Current,
    [Parameter(Mandatory)][string]$Base,
    [int]$Rounds = 5,
    [double]$Tolerance = 0.10,
    [string]$Entry,
    [string]$Packages,
    [string]$Target,
    [int]$Edits = 20
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$tmp = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { Join-Path $root 'target/tmp-zero-cost' }
New-Item -ItemType Directory -Force $tmp | Out-Null

function Median([double[]]$v) {
    $s = $v | Sort-Object
    $n = $s.Count
    if ($n % 2) { return $s[[int][math]::Floor($n / 2)] }
    return ($s[$n / 2 - 1] + $s[$n / 2]) / 2
}

# (1) The JS corpus: wall-clock time to compile every program.
function Corpus([string]$dir, [string]$output) {
    $exe = Join-Path $dir 'dartforge.exe'
    $programs = Get-ChildItem (Join-Path $root 'corpus/js') -Filter *.dart | Sort-Object Name
    $t = [Diagnostics.Stopwatch]::StartNew()
    foreach ($p in $programs) {
        & $exe compile-js $p.FullName -o $output *> $null
    }
    $t.Stop()
    return $t.Elapsed.TotalMilliseconds
}

# (2) Resident session: mean ms per body edit (the `measure` plateau).
function Session([string]$dir, [string]$entry, [string]$cfg, [string]$target) {
    $exe = Join-Path $dir 'measure.exe'
    $env:DARTFORGE_DEV_OUTPUT = Join-Path $tmp ('dev-' + (Split-Path $dir -Leaf))
    $a = @($entry)
    if ($cfg) { $a += $cfg } else { $a += '' }
    $a += @($target, "$Edits")
    $output = & $exe @a 2>&1 | Out-String
    # Tolerant to the console code page (the example's output is UTF-8). The
    # mean comes with two decimals (an integer in old binaries): in integer ms,
    # 1 ms at ~26 ms already exceeded the 3% tolerance. Binaries older than the
    # English output print `média N ms/edição`, which the base may still be.
    if ($output -notmatch '(?:mean|m\S*dia) (\d+(?:\.\d+)?) ms/edi') { throw "measure without the mean line:`n$output" }
    return [double]::Parse($Matches[1], [Globalization.CultureInfo]::InvariantCulture)
}

# Synthetic project: ~300 chained libraries, without build_runner.
function Synthetic {
    $p = Join-Path $tmp 'synthetic'
    if (Test-Path (Join-Path $p 'main.dart')) { return $p }
    New-Item -ItemType Directory -Force (Join-Path $p 'lib'), (Join-Path $p '.dart_tool') | Out-Null
    Set-Content (Join-Path $p 'pubspec.yaml') "name: synthetic`nenvironment:`n  sdk: ^3.6.0`n" -Encoding utf8
    Set-Content (Join-Path $p '.dart_tool/package_config.json') '{"configVersion":2,"packages":[{"name":"synthetic","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"}]}' -Encoding utf8
    for ($i = 0; $i -lt 300; $i++) {
        $imp = if ($i -gt 0) { "import 'l$($i - 1).dart';`n" } else { '' }
        $use = if ($i -gt 0) { "f$($i - 1)(x) + " } else { '' }
        Set-Content (Join-Path $p "lib/l$i.dart") "$imp`nclass C$i { final int v; C$i(this.v); int twice() => v * 2; }`nint f$i(int x) => ${use}C$i(x).twice();`n" -Encoding utf8
    }
    Set-Content (Join-Path $p 'main.dart') "import 'package:synthetic/l299.dart';`nvoid main() { print(f299(1)); }`n" -Encoding utf8
    return $p
}

function New-Measurements {
    $m = [ordered]@{}
    if (-not $Entry) {
        $m['JS corpus (ms)'] = @{ base = @(); current = @() }
        $m['body edit, 300 libraries (ms)'] = @{ base = @(); current = @() }
    }
    else {
        $m["body edit, $(Split-Path $Entry -Leaf) (ms)"] = @{ base = @(); current = @() }
    }
    return $m
}
if (-not $Entry) { $synth = Synthetic }

# One pass: $Rounds rounds, each measuring base and current in sequence
# (alternating order). The criterion is the MEDIAN OF THE PAIRED RATIOS
# current/base of each round: measurements taken side by side share the same
# runner drift, which comparing separate medians did not cancel (measured: up
# to 14% of noise between rounds; the gate failed commits that did not touch
# the compiler).
function Pass {
$measurements = New-Measurements
for ($r = 1; $r -le $Rounds; $r++) {
    # Alternates the order: whoever runs first pays for the cold disk.
    $order = if ($r % 2) { @('base', 'current') } else { @('current', 'base') }
    foreach ($which in $order) {
        $dir = if ($which -eq 'base') { $Base } else { $Current }
        if (-not $Entry) {
            $measurements['JS corpus (ms)'][$which] += Corpus $dir (Join-Path $tmp "js-$which")
            $measurements['body edit, 300 libraries (ms)'][$which] += Session $dir (Join-Path $synth 'main.dart') (Join-Path $synth '.dart_tool/package_config.json') (Join-Path $synth 'lib/l150.dart')
        }
        else {
            $target = if ($Target) { $Target } else { $Entry }
            $k = "body edit, $(Split-Path $Entry -Leaf) (ms)"
            $measurements[$k][$which] += Session $dir $Entry $Packages $target
        }
    }
    Write-Host "round $r of $Rounds"
}
return $measurements
}

# Evaluates a pass: returns (failed, table lines).
function Evaluate($measurements, [string]$title) {
    $failed = $false
    $md = @("### $title", '', "Rounds: $Rounds; tolerance: $([math]::Round($Tolerance * 100, 1))% over the median of the paired ratios.", '', '| measurement | base (median) | current (median) | paired ratio (median) | ratios per round |', '|---|---|---|---|---|')
    foreach ($k in $measurements.Keys) {
        $bs = $measurements[$k].base; $cs = $measurements[$k].current
        $ratios = @(for ($i = 0; $i -lt $bs.Count; $i++) { if ($bs[$i] -gt 0) { $cs[$i] / $bs[$i] } else { 1.0 } })
        $ratio = Median $ratios
        $ok = $ratio -le (1 + $Tolerance)
        if (-not $ok) { $failed = $true }
        $md += "| $k | $([math]::Round((Median $bs), 1)) | $([math]::Round((Median $cs), 1)) | $([math]::Round($ratio, 3))$(if (-not $ok) { ' **worse**' }) | $(($ratios | ForEach-Object { [math]::Round($_, 3) }) -join ', ') |"
    }
    return @{ failed = $failed; md = $md }
}

$first = Evaluate (Pass) 'Zero cost — current against base, paired ratios'
$output = $first.md
$failed = $first.failed
if ($failed) {
    # Confirmation: a real regression persists in a second pass; the runner's
    # bad luck does not. It only fails if both passes fail.
    Write-Host 'first pass above the tolerance; confirming with a second pass'
    $second = Evaluate (Pass) 'Confirmation (second pass)'
    $output += @('') + $second.md
    $failed = $second.failed
}
$text = $output -join "`n"
Write-Host $text
if ($env:GITHUB_STEP_SUMMARY) { Add-Content $env:GITHUB_STEP_SUMMARY $text -Encoding utf8 }
if ($failed) { Write-Host '::error::zero cost: current got slower than base beyond the tolerance, in two passes'; exit 1 }
exit 0
