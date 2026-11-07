param(
    [Parameter(Mandatory)][string]$Root,
    [Parameter(Mandatory)][ValidateRange(1, 32)][int]$ExpectedShards
)

$ErrorActionPreference = 'Stop'
# Other native jobs upload independently and must not change the shard sum.
$reports = @(Get-ChildItem $Root -Recurse -File -Filter report.txt |
    Where-Object { $_.Directory.Name -match '^report-native-([1-9][0-9]*)$' } |
    Sort-Object FullName)
$ids = @($reports | ForEach-Object {
    [int]($_.Directory.Name -replace '^report-native-', '')
} | Sort-Object)
if (($ids -join ',') -ne ((1..$ExpectedShards) -join ',')) {
    throw "Expected native shard IDs 1..$ExpectedShards; found: $($ids -join ',')"
}
foreach ($report in $reports) {
    $text = Get-Content $report.FullName -Raw -Encoding utf8
    $scores = [regex]::Matches($text, '(?m)^DartForge native:\s+([0-9]+)/([0-9]+) ok\r?$')
    if ($scores.Count -ne 1) {
        throw "Expected exactly one native scoreboard in $($report.FullName)"
    }
    $ok = [int]$scores[0].Groups[1].Value
    $total = [int]$scores[0].Groups[2].Value
    if ($total -le 0 -or $ok -gt $total) {
        throw "Invalid native scoreboard in $($report.FullName): $ok/$total"
    }
}
$reports
