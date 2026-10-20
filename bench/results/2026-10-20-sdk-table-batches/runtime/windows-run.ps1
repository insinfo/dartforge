$ErrorActionPreference='Stop';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$env:DART_UI_FONTS='C:\tools\ui\assets\fonts'
$shared='\\Mac\ssd_dock\MyRustProjects\dartforge\target\sdk-table-batches-validation'
$dest=Join-Path $env:LOCALAPPDATA 'DartForgeSDKTableBatches';New-Item -ItemType Directory -Force $dest | Out-Null
$artifacts=@{}
foreach($kind in @('fixture','explorer','context')) {
 $source=Join-Path $shared ('windows-x64-'+$kind+'.exe');$target=Join-Path $dest ($kind+'.exe');Copy-Item $source $target -Force
 $sha=(Get-FileHash $target).Hash.ToLowerInvariant();if($sha -ne (Get-FileHash $source).Hash.ToLowerInvariant()){throw 'copied executable hash mismatch'}
 $artifacts[$kind]=@{bytes=(Get-Item $target).Length;sha256=$sha}
}
$example='C:\tools\ui\examples\file_explorer'
$commands=@(
 @{name='fixture-normal';exe=(Join-Path $dest 'fixture.exe');args=@();stress=$false;marker="parent=45`nchild=45`n"},
 @{name='fixture-stress';exe=(Join-Path $dest 'fixture.exe');args=@();stress=$true;marker="parent=45`nchild=45`n"},
 @{name='explorer-aot';exe=(Join-Path $dest 'explorer.exe');args=@($example,'--lang=pt','--frames=2','--backend=win32','--presentation=direct3d11');stress=$false;marker=$null},
 @{name='context-aot';exe=(Join-Path $dest 'context.exe');args=@('--backend=win32','--presentation=direct3d11');stress=$false;marker='CONTEXT_RETARGET=PASS'}
)
$rows=@()
foreach($item in $commands) {
 $info=[Diagnostics.ProcessStartInfo]::new();$info.FileName=$item.exe;$info.Arguments=($item.args -join ' ');$info.WorkingDirectory=$example
 $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $info.StandardOutputEncoding=[Text.UTF8Encoding]::new($false);$info.StandardErrorEncoding=[Text.UTF8Encoding]::new($false);$info.EnvironmentVariables.Remove('DARTFORGE_GC_STRESS')
 if($item.stress){$info.EnvironmentVariables['DARTFORGE_GC_STRESS']='1'}
 $clock=[Diagnostics.Stopwatch]::StartNew();$process=[Diagnostics.Process]::new();$process.StartInfo=$info;[void]$process.Start()
 $stdout=$process.StandardOutput.ReadToEnd().Replace("`r`n","`n");$stderr=$process.StandardError.ReadToEnd();$process.WaitForExit();$clock.Stop()
 $rows+=@{name=$item.name;exe=$item.exe;args=$item.args;exit=$process.ExitCode;seconds=$clock.Elapsed.TotalSeconds;stdout=$stdout;stderr=$stderr;gc_stress=$item.stress}
 $report=@{artifacts=$artifacts;runs=$rows};ConvertTo-Json -Depth 10 -InputObject $report | Set-Content -Encoding UTF8 (Join-Path $dest 'results.json');Write-Output ($item.name+' '+$process.ExitCode)
 if($process.ExitCode -ne 0 -or $stderr.Length -ne 0){throw ($stdout+$stderr+' exit='+$process.ExitCode)}
 if($item.name.StartsWith('fixture')){if($stdout -ne $item.marker){throw 'fixture output mismatch'}}elseif($item.marker -and -not $stdout.Contains($item.marker)){throw 'context marker missing'}
}
ConvertTo-Json -Depth 10 -Compress -InputObject $report
