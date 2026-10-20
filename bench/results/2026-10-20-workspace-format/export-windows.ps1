$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$env:Path='C:\tools\llvm-22.1.8\bin;C:\tools\rust\cargo\bin;'+$env:Path
$env:DARTFORGE_LLVM_DIR='C:\tools\llvm-22.1.8';$env:LLVM_SYS_221_PREFIX=$env:DARTFORGE_LLVM_DIR
$env:CARGO_BUILD_JOBS='2';$env:CARGO_PROFILE_DEV_DEBUG='0';$env:CARGO_PROFILE_RELEASE_LTO='false'
$archive='\\Mac\ssd_dock\MyRustProjects\dartforge\target\workspace-format-validation\source-snapshot.tar.gz'
if((Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne 'd1d5957c984eafef5d55d704cf0c373896d9c0e37d9fbd416375e43ead13b594'){throw 'archive hash mismatch'}
$snapshot='C:\tools\df-image-'+[guid]::NewGuid().ToString('N').Substring(0,8)
New-Item -ItemType Directory -Path $snapshot | Out-Null
& tar.exe -xzf $archive -C $snapshot
if($LASTEXITCODE -ne 0){throw 'snapshot extraction failed'}
'//! Export the native runtime bundle of this frozen validation snapshot.
fn main() { dartforge_emit_native::cross::export(&std::path::PathBuf::from(std::env::args_os().nth(1).unwrap())).unwrap(); }' | Set-Content -Encoding UTF8 "$snapshot\crates\emit_native\examples\export_current_runtime.rs"
$snapshot | Set-Content C:\tools\forge-workspace-format-windows-snapshot.txt
Set-Location $snapshot
$ErrorActionPreference='Continue'
& cargo build --locked -p dartforge-emit-native --example export_current_runtime --target x86_64-pc-windows-msvc --target-dir C:\tools\dartforge\target\native-x64 *> C:\tools\forge-workspace-format-windows-build-final.log
$buildCode=$LASTEXITCODE
$ErrorActionPreference='Stop'
if($buildCode -ne 0){Get-Content C:\tools\forge-workspace-format-windows-build-final.log -Tail 25;exit $buildCode}
& C:\tools\dartforge\target\native-x64\x86_64-pc-windows-msvc\debug\examples\export_current_runtime.exe "$snapshot\bundle"
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
$dest='\\Mac\ssd_dock\MyRustProjects\dartforge\target\cross-bundles\workspace-format\windows-x64'
New-Item -ItemType Directory -Force $dest | Out-Null
Copy-Item "$snapshot\bundle\*" $dest -Recurse -Force
Get-Content "$snapshot\bundle\target.json" -Raw
