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

# crates/jit liga llvm-sys, que procura llvm-config na distribuicao completa do
# LLVM 22.1.x. O instalador reduzido (apenas LLVM-C.dll/.lib) nao serve. Sem esta
# variavel a build do workspace inteiro falha. Veja docs/JIT.md.
if (!$env:LLVM_SYS_221_PREFIX) {
    foreach ($prefix in @($env:DARTFORGE_LLVM_DIR, 'D:\DartSDKs\llvm\clang+llvm-22.1.8-x86_64-pc-windows-msvc')) {
        if ($prefix -and (Test-Path (Join-Path $prefix 'bin\llvm-config.exe'))) {
            $env:LLVM_SYS_221_PREFIX = $prefix
            break
        }
    }
}

# O JIT liga a biblioteca compartilhada da API C (LLVM-C.dll), e nao as
# bibliotecas estaticas: o pacote oficial de Windows as compila com CRT estatica,
# que conflita com a CRT dinamica do Rust. A DLL precisa estar alcancavel pelo
# carregador em tempo de execucao, tanto para os testes quanto para dartforge run.
if ($env:LLVM_SYS_221_PREFIX) {
    $llvmBin = Join-Path $env:LLVM_SYS_221_PREFIX 'bin'
    if ((Test-Path (Join-Path $llvmBin 'LLVM-C.dll')) -and ($env:Path -split [IO.Path]::PathSeparator) -notcontains $llvmBin) {
        $env:Path = $llvmBin + [IO.Path]::PathSeparator + $env:Path
    }
}
