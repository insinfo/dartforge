$ErrorActionPreference='Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$source='\\Mac\ssd_dock\MyRustProjects\dartforge\target\batch-tables-validation\cross-windows-x64.exe';$sha='68aa90115a142360151f32a41b2c3658da31e020668211d1ca3ca0a3c0bfa35b'
$dest=Join-Path $env:LOCALAPPDATA ('DartForgeBatchTables\'+$sha+'.exe')
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
ConvertTo-Json -Depth 8 -Compress -InputObject $rows
