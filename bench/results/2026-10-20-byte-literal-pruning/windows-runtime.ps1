$ErrorActionPreference='Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$source='\\Mac\ssd_dock\MyRustProjects\dartforge\target\byte-literal-pruning-validation\cross-windows-x64.exe';$sha='582a005e11f221717ea5aafd69190d6af7f7904a0b46d02f622f10d03b75c00d'
$dest=Join-Path $env:LOCALAPPDATA ('DartForgeByteLiteralPruning\'+$sha+'.exe')
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
[IO.File]::WriteAllText('\\Mac\ssd_dock\MyRustProjects\dartforge\target\byte-literal-pruning-validation\windows-runtime.json',$json,[Text.UTF8Encoding]::new($false))
$json
