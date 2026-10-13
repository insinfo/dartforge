# Shortcut for the heavy CI (.github/workflows/heavy.yml): dispatch, follow
# and read the scoreboard without running anything heavy on this machine. See ESTADO.md §3.
#
#   pwsh scripts/ci.ps1 -Front native              # pushes HEAD to ci/native (runs `all`)
#   pwsh scripts/ci.ps1 -Front native -Follow      # ... and follows it to the end
#   pwsh scripts/ci.ps1 -Suite native [-Ref ci/native] [-Shards 8]
#                                                  # workflow_dispatch (requires heavy.yml on main)
#   pwsh scripts/ci.ps1 -Follow [-Ref ci/native | -Run <id>]
#   pwsh scripts/ci.ps1 -Scoreboard <run-id>       # scoreboard of each job (downloads the small artifacts)
#   pwsh scripts/ci.ps1 -List                      # recent runs of every ci/** branch, with scoreboard
#
# Convention: each work front has its own ci/<front> branch. Runs of
# different branches never cancel each other; a new run on the SAME branch
# cancels the previous one. The ci/** branches are disposable — the push is forced.
param(
    [string]$Front,
    [ValidateSet('all', 'js', 'production', 'native', 'determinism', 'modern', 'macros', 'zero-cost', 'analysis')][string]$Suite,
    [string]$Ref,
    [int]$Shards = 0,
    [switch]$Follow,
    [string]$Run,
    [string]$Scoreboard,
    [switch]$Keep,
    [switch]$List
)
$ErrorActionPreference = 'Stop'
$workflow = 'heavy.yml'
$repo = gh repo view --json nameWithOwner -q .nameWithOwner
if (!$repo) { throw 'gh not authenticated or outside the repository' }

function Run-Of-Branch([string]$branch, [string]$sha) {
    # Waits for GitHub to register the run (a few seconds after the push).
    for ($i = 0; $i -lt 30; $i++) {
        $runs = gh run list --workflow $workflow --branch $branch -L 5 --json databaseId,headSha,status | ConvertFrom-Json
        $r = if ($sha) { $runs | Where-Object headSha -eq $sha | Select-Object -First 1 } else { $runs | Select-Object -First 1 }
        if ($r) { return $r.databaseId }
        Start-Sleep -Seconds 4
    }
    throw "no run of $workflow on $branch" + $(if ($sha) { " for $sha" } else { '' })
}

# Scoreboard published by each job as a `::notice title=Scoreboard <job>::…` annotation.
function Scoreboards([string]$id) {
    $jobs = (gh run view $id --json jobs | ConvertFrom-Json).jobs
    foreach ($j in $jobs) {
        $notes = gh api "repos/$repo/check-runs/$($j.databaseId)/annotations" 2>$null | ConvertFrom-Json
        foreach ($n in @($notes) | Where-Object { $_.title -like 'Scoreboard*' }) {
            [pscustomobject]@{ job = $j.name; scoreboard = $n.message }
        }
    }
}

$targetBranch = $null
$targetSha = $null

if ($Front) {
    if ($Front -notmatch '^[A-Za-z0-9._-]+$') { throw "invalid front name: $Front" }
    $targetBranch = "ci/$Front"
    $targetSha = git rev-parse HEAD
    git push --force origin "HEAD:refs/heads/$targetBranch"
    if ($LASTEXITCODE -ne 0) { throw 'git push failed' }
    Write-Host ("pushed $targetSha to $targetBranch" + ' — heavy.yml runs the "all" suite')
}

if ($Suite) {
    $dispatchRef = if ($Ref) { $Ref } elseif ($targetBranch) { $targetBranch } else { git branch --show-current }
    $a = @('workflow', 'run', $workflow, '--ref', $dispatchRef, '-f', "suite=$Suite")
    if ($Shards -gt 0) { $a += @('-f', "shards=$Shards") }
    gh @a
    if ($LASTEXITCODE -ne 0) { throw 'gh workflow run failed (is heavy.yml already on main? workflow_dispatch requires it)' }
    $targetBranch = $dispatchRef
    $targetSha = $null
    Start-Sleep -Seconds 5
}

if ($Follow) {
    $id = if ($Run) { $Run } else {
        $b = if ($targetBranch) { $targetBranch } elseif ($Ref) { $Ref } else { git branch --show-current }
        Run-Of-Branch $b $targetSha
    }
    Write-Host "following https://github.com/$repo/actions/runs/$id"
    gh run watch $id --compact --exit-status --interval 30
    $status = $LASTEXITCODE
    Scoreboards $id | Format-Table -AutoSize | Out-String | Write-Host
    exit $status
}

if ($Scoreboard) {
    gh run view $Scoreboard
    # Outside %TEMP%: C: is small and the downloads piled up (10 GB in one day
    # added to other leftovers). It goes to the repository's target/ (on D:,
    # ignored by git) and is deleted after being printed, unless -Keep.
    $dir = Join-Path $PSScriptRoot "..\target\ci-scoreboard\$Scoreboard"
    if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
    # The report-* and scoreboard-* artifacts are small text; the binaries do not come.
    gh run download $Scoreboard -D $dir -p 'report-*' -p 'scoreboard-*'
    Get-ChildItem $dir -Recurse -Filter scoreboard.md | Sort-Object FullName | ForEach-Object {
        Write-Host "`n=== $($_.Directory.Name) ===" -ForegroundColor Cyan
        Get-Content $_ -Encoding utf8 | Write-Host
    }
    if ($Keep) {
        Write-Host "`nfull reports in $dir"
    } else {
        Remove-Item -Recurse -Force $dir
        Write-Host "`n(reports deleted; use -Keep to keep them in target/ci-scoreboard)"
    }
}

if ($List) {
    $runs = gh run list --workflow $workflow -L 60 --json databaseId,headBranch,status,conclusion,createdAt,event,headSha | ConvertFrom-Json
    # The latest of each ci/** branch (and of main, which is the schedule).
    $latest = $runs | Where-Object { $_.headBranch -like 'ci/*' -or $_.headBranch -eq 'main' } |
        Group-Object headBranch | ForEach-Object { $_.Group | Sort-Object createdAt -Descending | Select-Object -First 1 }
    foreach ($r in $latest | Sort-Object createdAt -Descending) {
        $state = if ($r.status -eq 'completed') { $r.conclusion } else { $r.status }
        Write-Host ("{0,-22} {1,-12} {2}  {3}  https://github.com/{4}/actions/runs/{5}" -f $r.headBranch, $state, $r.createdAt, $r.headSha.Substring(0, 8), $repo, $r.databaseId) -ForegroundColor Cyan
        foreach ($p in Scoreboards $r.databaseId) { Write-Host ("    {0,-32} {1}" -f $p.job, $p.scoreboard) }
    }
}

if (!$Front -and !$Suite -and !$Follow -and !$Scoreboard -and !$List) {
    Get-Content $PSCommandPath | Select-Object -Skip 2 -First 14
}
