$ErrorActionPreference='Stop';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$env:Path='C:\tools\llvm-22.1.8\bin;C:\tools\rust\cargo\bin;'+$env:Path
$env:DARTFORGE_LLVM_DIR='C:\tools\llvm-22.1.8';$env:LLVM_SYS_221_PREFIX=$env:DARTFORGE_LLVM_DIR
$env:CARGO_BUILD_JOBS='1';$env:CARGO_PROFILE_DEV_DEBUG='0';$env:CARGO_PROFILE_RELEASE_OPT_LEVEL='1';$env:CARGO_PROFILE_RELEASE_DEBUG='0';$env:CARGO_PROFILE_RELEASE_LTO='false';$env:DARTFORGE_RUNTIME_NO_PRECOMPILE='0'
$snapshot=(Get-Content C:\tools\forge-current-desktop-windows-snapshot.txt -Raw).Trim();Set-Location $snapshot
$ErrorActionPreference='Continue'
& cargo build --release --locked -p dartforge-cli --features jit --target x86_64-pc-windows-msvc --target-dir C:\tools\dartforge\target\native-x64 *> C:\tools\forge-current-desktop-windows-build.log
$code=$LASTEXITCODE;$ErrorActionPreference='Stop'
if($code -ne 0){Get-Content C:\tools\forge-current-desktop-windows-build.log -Tail 35;exit $code}
$compiler='C:\tools\dartforge\target\native-x64\x86_64-pc-windows-msvc\release\dartforge-current-desktop.exe'
Copy-Item C:\tools\dartforge\target\native-x64\x86_64-pc-windows-msvc\release\dartforge.exe $compiler
@{binary=$compiler;sha256=(Get-FileHash $compiler).Hash.ToLowerInvariant();bytes=(Get-Item $compiler).Length} | ConvertTo-Json | Set-Content -Encoding UTF8 '\\Mac\ssd_dock\MyRustProjects\dartforge\target\current-desktop-validation\windows-compiler.json'
