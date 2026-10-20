$ErrorActionPreference='Stop';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$env:Path='C:\tools\llvm-22.1.8\bin;C:\tools\rust\cargo\bin;'+$env:Path
$env:DARTFORGE_LLVM_DIR='C:\tools\llvm-22.1.8';$env:DART_UI_FONTS='C:\tools\ui\assets\fonts'
$compiler=(Get-Content C:\tools\forge-batch-tables-windows-ui-compiler.txt -Raw).Trim()
$shared='\\Mac\ssd_dock\MyRustProjects\dartforge\target\batch-tables-validation'
$dest=Join-Path $env:LOCALAPPDATA 'DartForgeTableBatchesUI';New-Item -ItemType Directory -Force $dest | Out-Null
foreach($name in @('windows-x64-before-explorer.exe','windows-x64-after-explorer.exe','windows-x64-after-context.exe')) {
 Copy-Item (Join-Path $shared $name) (Join-Path $dest $name) -Force
 if((Get-FileHash (Join-Path $shared $name)).Hash -ne (Get-FileHash (Join-Path $dest $name)).Hash){throw 'copied executable hash mismatch'}
}
$example='C:\tools\ui\examples\file_explorer';$sdk='C:\tools\dart\3.6.2\dart-sdk\lib';$packages=$example+'\.dart_tool\package_config.json'
$commands=@(
 @{name='explorer-before-aot';exe=(Join-Path $dest 'windows-x64-before-explorer.exe');args=@($example,'--lang=pt','--frames=2','--backend=win32','--presentation=direct3d11');marker=$null},
 @{name='explorer-after-aot';exe=(Join-Path $dest 'windows-x64-after-explorer.exe');args=@($example,'--lang=pt','--frames=2','--backend=win32','--presentation=direct3d11');marker=$null},
 @{name='context-after-aot';exe=(Join-Path $dest 'windows-x64-after-context.exe');args=@('--backend=win32','--presentation=direct3d11');marker='CONTEXT_RETARGET=PASS'},
 @{name='explorer-after-jit';exe=$compiler;args=@('run',($example+'\bin\file_explorer.dart'),'--sdk',$sdk,'--packages',$packages,'--',$example,'--lang=pt','--frames=2','--backend=win32','--presentation=direct3d11');marker=$null},
 @{name='context-after-jit';exe=$compiler;args=@('run',($example+'\tool\context_menu_retarget_smoke.dart'),'--sdk',$sdk,'--packages',$packages,'--','--backend=win32','--presentation=direct3d11');marker='CONTEXT_RETARGET=PASS'}
)
$rows=@()
foreach($item in $commands) {
 $info=[Diagnostics.ProcessStartInfo]::new();$info.FileName=$item.exe;$info.Arguments=($item.args -join ' ');$info.WorkingDirectory=$example
 $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $info.StandardOutputEncoding=[Text.UTF8Encoding]::new($false);$info.StandardErrorEncoding=[Text.UTF8Encoding]::new($false);$info.EnvironmentVariables.Remove('DARTFORGE_GC_STRESS')
 $clock=[Diagnostics.Stopwatch]::StartNew();$process=[Diagnostics.Process]::new();$process.StartInfo=$info;[void]$process.Start()
 $stdout=$process.StandardOutput.ReadToEnd();$stderr=$process.StandardError.ReadToEnd();$process.WaitForExit();$clock.Stop()
 $rows+=@{name=$item.name;exe=$item.exe;args=$item.args;exit=$process.ExitCode;seconds=$clock.Elapsed.TotalSeconds;stdout=$stdout;stderr=$stderr}
 $report=@{compiler=$compiler;compiler_sha256=(Get-FileHash $compiler).Hash.ToLowerInvariant();runs=$rows}
 ConvertTo-Json -Depth 10 -InputObject $report | Set-Content -Encoding UTF8 (Join-Path $dest 'results.json')
 Write-Output ($item.name+' '+$process.ExitCode)
 if($process.ExitCode -ne 0 -or ($item.marker -and -not $stdout.Contains($item.marker)) -or $stderr.Contains('Bad state:') -or $stderr.Contains('Unhandled exception')){throw ($stdout+$stderr+' exit='+$process.ExitCode)}
}
ConvertTo-Json -Depth 10 -Compress -InputObject $report
