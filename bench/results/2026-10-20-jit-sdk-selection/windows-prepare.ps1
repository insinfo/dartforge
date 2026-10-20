$ErrorActionPreference='Stop';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$shared='\\Mac\ssd_dock\MyRustProjects\dartforge\target\current-desktop-validation'
$archive=Join-Path $shared 'source-fixed-snapshot.tar.gz'
if((Get-FileHash $archive).Hash.ToLowerInvariant() -ne 'c33aa08c8f0654ae8c562c3fb3662389fd22a7640841db88f581fea3cac10e8b'){throw 'Source archive hash mismatch'}
$snapshot='C:\tools\dfcd-'+[Guid]::NewGuid().ToString('N').Substring(0,8)
New-Item -ItemType Directory $snapshot | Out-Null
& tar.exe -xf $archive -C $snapshot
if($LASTEXITCODE -ne 0){throw 'Source extraction failed'}
$files=Get-Content (Join-Path $shared 'fixed-source-files.json') -Raw | ConvertFrom-Json
foreach($item in $files){if((Get-FileHash -LiteralPath (Join-Path $snapshot $item.path)).Hash.ToLowerInvariant() -ne $item.sha256){throw ('Source hash mismatch: '+$item.path)}}
$snapshot | Set-Content C:\tools\forge-current-desktop-windows-snapshot.txt
@{snapshot=$snapshot;files=$files.Count;archive_sha256=(Get-FileHash $archive).Hash.ToLowerInvariant()} | ConvertTo-Json | Set-Content -Encoding UTF8 (Join-Path $shared 'windows-source-verification.json')
Get-PSDrive C | Select-Object Name,Used,Free | ConvertTo-Json
Write-Output ('Verified '+$files.Count+' compiler source files')
