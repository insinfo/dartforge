$ErrorActionPreference='Stop';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$shared='\\Mac\ssd_dock\MyRustProjects\dartforge\target\current-desktop-validation'
$compiler='C:\tools\dartforge\target\native-x64\x86_64-pc-windows-msvc\release\dartforge-current-desktop.exe'
$snapshot=(Get-Content C:\tools\forge-current-desktop-windows-snapshot.txt -Raw).Trim();$fixture=Join-Path $snapshot 'crates\cli\tests\fixtures\jit_main_arguments.dart';$sdk='C:\tools\dart\3.6.2\dart-sdk\lib'
$poison=Join-Path $env:LOCALAPPDATA 'DartForgeCurrentDesktop\invalid-sdk';$rows=@()
foreach($mode in @('run','reload','restart')) {
 $arguments=@($(if($mode -eq 'run'){'run'}else{'reload'}),'--sdk',$sdk,$fixture)
 if($mode -ne 'run'){$arguments+=@('--once')};if($mode -eq 'restart'){$arguments+=@('--restart')};$arguments+=@('--','--sdk','application-owned','--')
 $info=[Diagnostics.ProcessStartInfo]::new();$info.FileName=$compiler;$info.Arguments=($arguments -join ' ');$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $info.EnvironmentVariables['Path']='C:\tools\llvm-22.1.8\bin;'+$env:Path;$info.EnvironmentVariables['DARTFORGE_LLVM_DIR']='C:\tools\llvm-22.1.8';$info.EnvironmentVariables['DARTFORGE_SDK_LIB']=$poison;$info.EnvironmentVariables.Remove('DARTFORGE_SDK_DLL');$info.EnvironmentVariables.Remove('DART_SDK')
 $p=[Diagnostics.Process]::new();$p.StartInfo=$info;[void]$p.Start();$a=$p.StandardOutput.ReadToEndAsync();$b=$p.StandardError.ReadToEndAsync();$p.WaitForExit();$stdout=$a.Result;$stderr=$b.Result
 $rows+=@{mode=$mode;compiler=$compiler;compiler_sha256=(Get-FileHash $compiler).Hash.ToLowerInvariant();arguments=$arguments;exit=$p.ExitCode;stdout=$stdout;stderr=$stderr};$rows | ConvertTo-Json -Depth 10 | Set-Content -Encoding UTF8 (Join-Path $shared 'windows-sdk-modes.json')
 Write-Output ($mode+' '+$p.ExitCode)
 if($p.ExitCode -ne 0 -or $stdout.Replace("`r`n","`n") -ne "[`"--sdk`",`"application-owned`",`"--`"]`n"){throw ($mode+': '+$stderr+$stdout)}
}
