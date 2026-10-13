# Scoreboard of the differential harness in Markdown, for $GITHUB_STEP_SUMMARY.
#
# Reads one or more `dartforge-differential` reports (its stdout) and writes
# the scoreboard, the time and the failure grouping. With several reports (the
# shards of the native corpus) it ADDS the scoreboards and MERGES the failure
# groups by key — the same grouping the harness would give running the whole
# corpus on a single machine.
#
#   pwsh scripts/ci-scoreboard.ps1 -Report report.txt -Title 'JS development'
#   pwsh scripts/ci-scoreboard.ps1 -Report (ls rel -Recurse -Filter report.txt) -Title 'Native' -Expected 8
#   pwsh scripts/ci-scoreboard.ps1 -Report report.txt -Line     # only the scoreboard, on one line
param(
    [Parameter(Mandatory)][string[]]$Report,
    [string]$Title = 'Scoreboard',
    # How many reports should exist (shards); a missing one is said at the top.
    [int]$Expected = 0,
    # A single line (for the job's `::notice` annotation).
    [switch]$Line
)
$ErrorActionPreference = 'Stop'

# Scoreboard per executor: label -> [ok, total]. DDC×VM has its own format.
$scoreboard = [ordered]@{}
$ddc = $null
# Failure groups: section -> (key -> list of programs).
$groups = [ordered]@{}
$programs = 0
$seconds = @()
$dartBroken = @()
$pending = $null
# The `--jit-aot` section (JIT × AOT agreement and post-IR times), copied as is.
$jitAot = [System.Collections.Generic.List[string]]::new()
$read = 0

foreach ($file in $Report) {
    if (!(Test-Path $file)) { continue }
    $read++
    $lines = Get-Content -LiteralPath $file -Encoding utf8
    $section = $null
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $l = $lines[$i]
        if ($l -match '^(JIT × AOT: |JIT≠AOT |Post-IR times|  (JIT|AOT) |  ratio |  timed out in both)') { $jitAot.Add($l) }
        if ($l -match '^(DartForge [^:]+):\s+(\d+)/(\d+) ok') {
            $r = $Matches[1]
            if (!$scoreboard.Contains($r)) { $scoreboard[$r] = @(0, 0) }
            $scoreboard[$r] = @(($scoreboard[$r][0] + [int]$Matches[2]), ($scoreboard[$r][1] + [int]$Matches[3]))
            $section = $null
        } elseif ($l -match '^Pending \(PENDENTES\): (\d+) still fail, (\d+) passed') {
            $pending = @([int]$Matches[1], [int]$Matches[2])
        } elseif ($l -match '^DDC×VM: (\d+)/(\d+) match \(not counting (\d+)') {
            if (!$ddc) { $ddc = @(0, 0, 0) }
            $ddc = @(($ddc[0] + [int]$Matches[1]), ($ddc[1] + [int]$Matches[2]), ($ddc[2] + [int]$Matches[3]))
        } elseif ($l -match '^\((\d+) programs in ([\d.,]+) s\)') {
            $programs += [int]$Matches[1]
            $seconds += [double]($Matches[2] -replace ',', '.')
        } elseif ($l -match '^DART!\s+(\S+)') {
            $dartBroken += $Matches[1]
        } elseif ($l -match '^Failures by first stderr line (.+?) \(priority') {
            $section = $Matches[1]
            if (!$groups.Contains($section)) { $groups[$section] = [ordered]@{} }
        } elseif ($section -and $l -match '^\s{2}\s*\d+  (.*)$' -and ($i + 1) -lt $lines.Count -and $lines[$i + 1] -match '^\s{8}(\S.*)$') {
            $key = $l -replace '^\s*\d+  ', ''
            $names = $lines[$i + 1].Trim() -split ', '
            if (!$groups[$section].Contains($key)) { $groups[$section][$key] = [System.Collections.Generic.List[string]]::new() }
            $groups[$section][$key].AddRange([string[]]$names)
            $i++
        }
    }
}

# Corpus order: numeric prefix, then the name (like `corpus::list`).
function Order([string]$n) {
    $num = if ($n -match '^(\d+)') { [int]$Matches[1] } else { [int]::MaxValue }
    return '{0:D10}{1}' -f $num, $n
}

$summary = @($scoreboard.Keys | ForEach-Object { "$_ $($scoreboard[$_][0])/$($scoreboard[$_][1])" })
if ($Line) {
    $txt = if ($summary) { $summary -join '; ' } else { 'no scoreboard in the report' }
    if ($Expected -gt 0 -and $read -lt $Expected) { $txt += " (only $read of $Expected shards)" }
    Write-Output $txt
    return
}

$out = [System.Collections.Generic.List[string]]::new()
$out.Add("### $Title")
$out.Add('')
if ($Expected -gt 0 -and $read -lt $Expected) {
    $out.Add("> **Warning: only $read of $Expected reports arrived** — the scoreboard below is partial; see the jobs that failed.")
    $out.Add('')
}
if ($read -eq 0) {
    $out.Add('No report found — the harness did not get to write the report.')
} else {
    $out.Add('| executor | scoreboard |')
    $out.Add('| --- | ---: |')
    foreach ($r in $scoreboard.Keys) { $out.Add("| $r | **$($scoreboard[$r][0])/$($scoreboard[$r][1])** |") }
    if ($pending) { $out.Add("| pending (PENDENTES) | $($pending[0]) still fail, $($pending[1]) passed |") }
    if ($ddc) { $out.Add("| DDC×VM (oracles) | $($ddc[0])/$($ddc[1] - $ddc[2]) (+$($ddc[2]) with a declared divergence) |") }
    $out.Add('')
    if ($seconds.Count -eq 1) {
        $out.Add("$programs programs in $([math]::Round($seconds[0])) s.")
    } elseif ($seconds.Count -gt 1) {
        $max = ($seconds | Measure-Object -Maximum).Maximum
        $sum = ($seconds | Measure-Object -Sum).Sum
        $out.Add("$programs programs in $($seconds.Count) reports; the slowest took $([math]::Round($max)) s, sum $([math]::Round($sum)) s.")
    }
    if ($dartBroken) {
        $out.Add('')
        # Expected in the web-only programs (`// diverge-ddc:`), whose reference is DDC.
        $out.Add('`dart run` exited with a code ≠ 0 in ' + $dartBroken.Count + ' (the report''s `DART!` lines): ' + (($dartBroken | Sort-Object { Order $_ }) -join ', '))
    }
    if ($jitAot.Count -gt 0) {
        $out.Add('')
        $out.Add('```text')
        foreach ($l in $jitAot) { $out.Add($l) }
        $out.Add('```')
    }
    foreach ($section in $groups.Keys) {
        $g = $groups[$section]
        $total = ($g.Values | ForEach-Object { $_.Count } | Measure-Object -Sum).Sum
        $out.Add('')
        $out.Add("<details><summary>Failures $section by first stderr line: $total programs in $($g.Count) groups</summary>")
        $out.Add('')
        $out.Add('```text')
        foreach ($key in ($g.Keys | Sort-Object @{ Expression = { $g[$_].Count }; Descending = $true }, @{ Expression = { $_ } })) {
            $out.Add(('{0,6}  {1}' -f $g[$key].Count, $key))
            $out.Add('        ' + (($g[$key] | Sort-Object { Order $_ }) -join ', '))
        }
        $out.Add('```')
        $out.Add('')
        $out.Add('</details>')
    }
}
$out -join "`n"
