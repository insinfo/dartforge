$ErrorActionPreference='Stop';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$env:Path='C:\tools\llvm-22.1.8\bin;C:\tools\rust\cargo\bin;'+$env:Path
$env:DARTFORGE_LLVM_DIR='C:\tools\llvm-22.1.8';$env:DART_UI_FONTS='C:\tools\ui\assets\fonts'
$shared='\\Mac\ssd_dock\MyRustProjects\dartforge\target\current-desktop-validation'
$compiler='C:\tools\dartforge\target\native-x64\x86_64-pc-windows-msvc\release\dartforge-current-desktop.exe'
$dest=Join-Path $env:LOCALAPPDATA 'DartForgeCurrentDesktop';New-Item -ItemType Directory -Force $dest | Out-Null
$transfers=@()
foreach($name in @('windows-x64-explorer.exe','windows-x64-context.exe')) {
 $source=Join-Path $shared $name;$target=Join-Path $dest $name;Copy-Item $source $target -Force
 $hash=(Get-FileHash $source).Hash.ToLowerInvariant();if($hash -ne (Get-FileHash $target).Hash.ToLowerInvariant()){throw 'Copied executable hash mismatch'}
 $transfers+=@{source=$source;destination=$target;sha256=$hash}
}
$poison=Join-Path $dest 'invalid-sdk';New-Item -ItemType Directory -Force $poison | Out-Null
[IO.File]::WriteAllText((Join-Path $poison 'libraries.json'),'not a library manifest')
$example='C:\tools\ui\examples\file_explorer';$sdk='C:\tools\dart\3.6.2\dart-sdk\lib';$packages=$example+'\.dart_tool\package_config.json'
$commands=@(
 @{name='explorer-cross-aot';exe=(Join-Path $dest 'windows-x64-explorer.exe');args=@($example,'--lang=pt','--frames=2','--backend=win32','--presentation=direct3d11');marker=$null},
 @{name='context-cross-aot';exe=(Join-Path $dest 'windows-x64-context.exe');args=@('--backend=win32','--presentation=direct3d11');marker='CONTEXT_RETARGET=PASS'},
 @{name='explorer-native-jit';exe=$compiler;args=@('run','--sdk',$sdk,'--packages',$packages,($example+'\bin\file_explorer.dart'),'--',$example,'--lang=pt','--frames=2','--backend=win32','--presentation=direct3d11');marker=$null},
 @{name='context-native-jit';exe=$compiler;args=@('run','--sdk',$sdk,'--packages',$packages,($example+'\tool\context_menu_retarget_smoke.dart'),'--','--backend=win32','--presentation=direct3d11');marker='CONTEXT_RETARGET=PASS'}
)
$rows=@();$report=@{compiler=$compiler;compiler_sha256=(Get-FileHash $compiler).Hash.ToLowerInvariant();runs=$rows;transfers=$transfers;ambient_sdk=$poison}
foreach($item in $commands) {
 $info=[Diagnostics.ProcessStartInfo]::new();$info.FileName=$item.exe;$info.Arguments=($item.args -join ' ');$info.WorkingDirectory=$example
 $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $info.StandardOutputEncoding=[Text.UTF8Encoding]::new($false);$info.StandardErrorEncoding=[Text.UTF8Encoding]::new($false)
 $info.EnvironmentVariables.Remove('DARTFORGE_GC_STRESS');$info.EnvironmentVariables.Remove('DARTFORGE_SDK_DLL');$info.EnvironmentVariables['DARTFORGE_SDK_LIB']=$poison
 $clock=[Diagnostics.Stopwatch]::StartNew();$process=[Diagnostics.Process]::new();$process.StartInfo=$info;[void]$process.Start()
 $report.state=@{status='running';name=$item.name;pid=$process.Id};$report | ConvertTo-Json -Depth 10 | Set-Content -Encoding UTF8 (Join-Path $shared 'windows-ui-report.json')
 $stdoutTask=$process.StandardOutput.ReadToEndAsync();$stderrTask=$process.StandardError.ReadToEndAsync();$process.WaitForExit();$stdout=$stdoutTask.Result;$stderr=$stderrTask.Result;$clock.Stop()
 $rows+=@{name=$item.name;exe=$item.exe;args=$item.args;exit=$process.ExitCode;seconds=$clock.Elapsed.TotalSeconds;stdout=$stdout;stderr=$stderr};$report.runs=$rows
 $report | ConvertTo-Json -Depth 10 | Set-Content -Encoding UTF8 (Join-Path $shared 'windows-ui-report.json')
 Write-Output ($item.name+' '+$process.ExitCode)
 if($process.ExitCode -ne 0 -or ($item.marker -and -not $stdout.Contains($item.marker)) -or $stderr.Length -ne 0){throw ($stdout+$stderr+' exit='+$process.ExitCode)}
}
$report.state=@{status='finished'};$report | ConvertTo-Json -Depth 10 | Set-Content -Encoding UTF8 (Join-Path $shared 'windows-ui-report.json')
