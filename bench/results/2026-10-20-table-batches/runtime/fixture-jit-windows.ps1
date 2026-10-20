$ErrorActionPreference='Stop';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$env:Path='C:\tools\llvm-22.1.8\bin;C:\tools\rust\cargo\bin;'+$env:Path
$env:DARTFORGE_LLVM_DIR='C:\tools\llvm-22.1.8'
$snapshot=(Get-Content C:\tools\forge-batch-tables-windows-snapshot.txt -Raw).Trim()
$rows=@()
foreach($stress in @($false,$true)) {
 $info=[Diagnostics.ProcessStartInfo]::new();$info.FileName='C:\tools\dartforge\target\native-x64\x86_64-pc-windows-msvc\debug\dartforge-batch-tables.exe'
 $info.Arguments='run --sdk C:\tools\dart\3.6.2\dart-sdk\lib '+$snapshot+'\crates\emit_native\tests\fixtures\table_batch\main.dart'
 $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $info.StandardOutputEncoding=[Text.UTF8Encoding]::new($false);$info.StandardErrorEncoding=[Text.UTF8Encoding]::new($false)
 $info.EnvironmentVariables.Remove('DARTFORGE_GC_STRESS');if($stress){$info.EnvironmentVariables['DARTFORGE_GC_STRESS']='1'}
 $p=[Diagnostics.Process]::new();$p.StartInfo=$info;[void]$p.Start();$stdout=$p.StandardOutput.ReadToEnd();$stderr=$p.StandardError.ReadToEnd();$p.WaitForExit()
 if($p.ExitCode -ne 0 -or $stdout.Replace("`r`n","`n") -ne "parent=45`nchild=45`n" -or $stderr){throw ($stdout+$stderr+' exit='+$p.ExitCode)}
 $rows+=@{stress=$stress;exit=$p.ExitCode;stdout=$stdout;stderr=$stderr}
}
ConvertTo-Json -Depth 8 -Compress -InputObject $rows
