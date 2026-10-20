$ErrorActionPreference='Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$source='\\Mac\ssd_dock\MyRustProjects\dartforge\target\workspace-format-validation\cross-windows-x64.exe';$sha='19f3a421a263cab7d8bf783c0442edd398e938e2576b010f0379b9e6833e4529'
$dest=Join-Path $env:LOCALAPPDATA ('DartForgeWorkspaceFormat\'+$sha+'.exe')
New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
if((Get-FileHash $source -Algorithm SHA256).Hash.ToLowerInvariant() -ne $sha){throw 'source hash mismatch'}
Copy-Item -LiteralPath $source -Destination $dest -Force
$rows=@()
foreach($stress in @($false,$true)) {
 $info=[Diagnostics.ProcessStartInfo]::new();$info.FileName=$dest;$info.UseShellExecute=$false
 $info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $info.StandardOutputEncoding=[Text.UTF8Encoding]::new($false);$info.StandardErrorEncoding=[Text.UTF8Encoding]::new($false)
 $info.EnvironmentVariables.Remove('DARTFORGE_GC_STRESS')
 if($stress){$info.EnvironmentVariables['DARTFORGE_GC_STRESS']='1'}
 $p=[Diagnostics.Process]::new();$p.StartInfo=$info;[void]$p.Start()
 $stdout=$p.StandardOutput.ReadToEnd();$stderr=$p.StandardError.ReadToEnd();$p.WaitForExit()
 $rows+=@{stress=$stress;exit=$p.ExitCode;stdout=$stdout;stderr=$stderr;sha256=(Get-FileHash $dest -Algorithm SHA256).Hash.ToLowerInvariant()}
}
$json=ConvertTo-Json -Depth 8 -Compress -InputObject $rows
[IO.File]::WriteAllText('\\Mac\ssd_dock\MyRustProjects\dartforge\target\workspace-format-validation\windows-runtime.json',$json,[Text.UTF8Encoding]::new($false))
$json
