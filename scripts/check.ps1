$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/env.ps1"
$previousRustdocFlags = $env:RUSTDOCFLAGS
Push-Location (Split-Path $PSScriptRoot)
try {
 cargo fmt --all -- --check
 if ($LASTEXITCODE) { throw 'Formatting failed' }
 cargo clippy --locked --workspace --all-targets -- -D warnings
 if ($LASTEXITCODE) { throw 'Clippy failed' }
 cargo test --locked --workspace
 if ($LASTEXITCODE) { throw 'Tests failed' }
 $env:RUSTDOCFLAGS = '-D warnings'
 cargo doc --locked --workspace --no-deps --document-private-items
 if ($LASTEXITCODE) { throw 'Documentation failed' }
 cargo build --locked --workspace --release
 if ($LASTEXITCODE) { throw 'Build failed' }
} finally {
 $env:RUSTDOCFLAGS = $previousRustdocFlags
 Pop-Location
}
