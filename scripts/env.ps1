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
