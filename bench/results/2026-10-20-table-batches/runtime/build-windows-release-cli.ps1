$ErrorActionPreference='Stop';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$env:Path='C:\tools\llvm-22.1.8\bin;C:\tools\rust\cargo\bin;'+$env:Path
$env:DARTFORGE_LLVM_DIR='C:\tools\llvm-22.1.8';$env:LLVM_SYS_221_PREFIX=$env:DARTFORGE_LLVM_DIR
$env:CARGO_BUILD_JOBS='1';$env:CARGO_PROFILE_DEV_DEBUG='0';$env:CARGO_PROFILE_RELEASE_OPT_LEVEL='1';$env:CARGO_PROFILE_RELEASE_DEBUG='0';$env:CARGO_PROFILE_RELEASE_LTO='false';$env:DARTFORGE_RUNTIME_NO_PRECOMPILE='0'
$snapshot=(Get-Content C:\tools\forge-batch-tables-windows-snapshot.txt -Raw).Trim();Set-Location $snapshot
$ErrorActionPreference='Continue'
& cargo build --release --locked -p dartforge-cli --features jit --target x86_64-pc-windows-msvc --target-dir C:\tools\dartforge\target\native-x64 *> C:\tools\forge-batch-tables-windows-release-cli-build.log
$code=$LASTEXITCODE;$ErrorActionPreference='Stop'
if($code -ne 0){Get-Content C:\tools\forge-batch-tables-windows-release-cli-build.log -Tail 35;exit $code}
$compiler='C:\tools\dartforge\target\native-x64\x86_64-pc-windows-msvc\release\dartforge-batch-tables.exe'
Copy-Item C:\tools\dartforge\target\native-x64\x86_64-pc-windows-msvc\release\dartforge.exe $compiler -Force
Get-FileHash $compiler -Algorithm SHA256 | ConvertTo-Json -Compress
