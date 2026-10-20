$ErrorActionPreference='Stop'
$dest='\\Mac\ssd_dock\MyRustProjects\dartforge\target\batch-tables-validation'
foreach($name in @('build-final','cli-build','release-cli-build','short-release-cli-build')){ $p='C:\tools\forge-batch-tables-windows-'+$name+'.log';if(Test-Path $p){Copy-Item $p (Join-Path $dest ('windows-'+$name+'.log')) -Force}}
