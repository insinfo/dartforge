[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
Get-PSDrive C | Select-Object Used,Free | ConvertTo-Json -Compress
Get-Content C:\tools\forge-batch-tables-windows-release-cli-build.log | Select-String -Pattern '^error:|LLVM ERROR|fatal error|LINK :|could not|memory|space|archive at' | Select-Object -Last 12 | ForEach-Object { $_.Line }
