[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
Get-Process dartforge-batch-tables,powershell -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,CPU,WorkingSet64 | ConvertTo-Json -Compress
$p=Join-Path $env:LOCALAPPDATA 'DartForgeTableBatchesUI/results.json';if(Test-Path $p){Get-Content $p -Raw}
