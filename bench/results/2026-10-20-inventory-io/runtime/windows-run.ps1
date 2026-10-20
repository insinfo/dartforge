$ErrorActionPreference='Stop';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$shared='\\Mac\ssd_dock\MyRustProjects\dartforge\target\inventory-io-validation'
$source=Join-Path $shared 'after-windows-x64-fixture.exe';$dest=Join-Path $env:LOCALAPPDATA 'DartForgeInventoryIOFixture.exe';Copy-Item $source $dest -Force
$sha=(Get-FileHash $dest).Hash.ToLowerInvariant();if($sha -ne (Get-FileHash $source).Hash.ToLowerInvariant()){throw 'hash mismatch'}
$expected=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('cGFyZW50PTQ1CmNoaWxkPTQ1Cg=='));$rows=@()
foreach($stress in @($false,$true)) {
 $info=[Diagnostics.ProcessStartInfo]::new();$info.FileName=$dest;$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true;$info.StandardOutputEncoding=[Text.UTF8Encoding]::new($false);$info.StandardErrorEncoding=[Text.UTF8Encoding]::new($false);$info.EnvironmentVariables.Remove('DARTFORGE_GC_STRESS');if($stress){$info.EnvironmentVariables['DARTFORGE_GC_STRESS']='1'}
 $p=[Diagnostics.Process]::new();$p.StartInfo=$info;[void]$p.Start();$stdout=$p.StandardOutput.ReadToEnd().Replace("`r`n","`n");$stderr=$p.StandardError.ReadToEnd();$p.WaitForExit()
 $rows+=@{exit=$p.ExitCode;stdout=$stdout;stderr=$stderr;stress=$stress;sha256=$sha}
 ConvertTo-Json -Depth 6 -InputObject $rows | Set-Content -Encoding UTF8 (Join-Path $shared 'windows-result.json')
 if($p.ExitCode -ne 0 -or $stdout -ne $expected -or $stderr.Length -ne 0){throw ($stdout+$stderr+' mismatch')}
}
ConvertTo-Json -Depth 6 -Compress -InputObject $rows
