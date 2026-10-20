$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/env.ps1"
$previousRustdocFlags = $env:RUSTDOCFLAGS
Push-Location (Split-Path $PSScriptRoot)
try {
 & "$PSScriptRoot/check-test-prerequisites.ps1"
 cargo fmt --all -- --check
 if ($LASTEXITCODE) { throw 'Formatting failed' }
 cargo clippy --locked --workspace --all-targets -- -D warnings
 if ($LASTEXITCODE) { throw 'Clippy failed' }
 cargo test --locked --workspace --no-fail-fast -- --include-ignored
 if ($LASTEXITCODE) { throw 'Tests failed' }
 cargo test --locked -p dartforge-cli --features native,jit --no-fail-fast -- --include-ignored
 if ($LASTEXITCODE) { throw 'Native/JIT CLI tests failed' }
 $env:RUSTDOCFLAGS = '-D warnings'
 cargo doc --locked --workspace --no-deps --document-private-items
 if ($LASTEXITCODE) { throw 'Documentation failed' }
 cargo build --locked --workspace --release
 if ($LASTEXITCODE) { throw 'Build failed' }
} finally {
 $env:RUSTDOCFLAGS = $previousRustdocFlags
 Pop-Location
}
