[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$s=Get-Content C:\tools\forge-batch-tables-windows-release-cli-build.log -Raw;$i=$s.IndexOf('LINK : fatal');if($i -ge 0){$s.Substring($i,[Math]::Min(2600,$s.Length-$i))}
