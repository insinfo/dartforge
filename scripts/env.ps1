# Prefer explicitly configured toolchains. Fall back to the original local installation.
if (!$env:CARGO_HOME -and !$env:RUSTUP_HOME -and (Test-Path 'D:\Rust\cargo\bin\cargo.exe')) {
    $env:CARGO_HOME = 'D:\Rust\cargo'
    $env:RUSTUP_HOME = 'D:\Rust\rustup'
}
if ($env:CARGO_HOME) {
    $cargoBin = Join-Path $env:CARGO_HOME 'bin'
    if ((Test-Path $cargoBin) -and ($env:Path -split [IO.Path]::PathSeparator) -notcontains $cargoBin) {
        $env:Path = $cargoBin + [IO.Path]::PathSeparator + $env:Path
    }
}

# Prefer user configuration or PATH; use the verified local LLVM only as fallback.
if (!$env:DARTFORGE_CLANG -and !(Get-Command clang -ErrorAction SilentlyContinue) -and (Test-Path 'D:\LLVM\22.1.8\bin\clang.exe')) {
    $env:DARTFORGE_CLANG = 'D:\LLVM\22.1.8\bin\clang.exe'
}
