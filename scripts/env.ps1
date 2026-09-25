# Prefer explicitly configured toolchains. Use the SSD copy when available.
if (!$env:CARGO_HOME -and !$env:RUSTUP_HOME) {
    foreach ($rustRoot in @('E:\Rust', 'D:\Rust')) {
        if (Test-Path (Join-Path $rustRoot 'cargo\bin\cargo.exe')) {
            $env:CARGO_HOME = Join-Path $rustRoot 'cargo'
            $env:RUSTUP_HOME = Join-Path $rustRoot 'rustup'
            break
        }
    }
}
if ($env:CARGO_HOME) {
    $cargoBin = Join-Path $env:CARGO_HOME 'bin'
    if ((Test-Path $cargoBin) -and ($env:Path -split [IO.Path]::PathSeparator) -notcontains $cargoBin) {
        $env:Path = $cargoBin + [IO.Path]::PathSeparator + $env:Path
    }
}

# Prefer user configuration or PATH; use the verified local LLVM only as fallback.
if (!$env:DARTFORGE_CLANG -and !(Get-Command clang -ErrorAction SilentlyContinue)) {
    foreach ($clang in @('E:\llvm\clang+llvm-22.1.8-x86_64-pc-windows-msvc\bin\clang.exe', 'D:\LLVM\22.1.8\bin\clang.exe')) {
        if (Test-Path $clang) {
            $env:DARTFORGE_CLANG = $clang
            break
        }
    }
}

# crates/jit liga llvm-sys, que procura llvm-config na distribuicao completa do
# LLVM 22.1.x. O instalador reduzido (apenas LLVM-C.dll/.lib) nao serve. Sem esta
# variavel a build do workspace inteiro falha. Veja docs/JIT.md.
if (!$env:LLVM_SYS_221_PREFIX) {
    foreach ($prefix in @($env:DARTFORGE_LLVM_DIR, 'E:\llvm\clang+llvm-22.1.8-x86_64-pc-windows-msvc')) {
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

# Use the SDKs on the SSD unless the caller selected another installation.
if (!$env:DART_SDK -and (Test-Path 'E:\DartSDKs\3.6.2\bin\dart.exe')) {
    $env:DART_SDK = 'E:\DartSDKs\3.6.2'
}
if (!$env:DARTFORGE_SDK_LIB -and (Test-Path 'E:\DartSDKs\3.6.2\lib\libraries.json')) {
    $env:DARTFORGE_SDK_LIB = 'E:\DartSDKs\3.6.2\lib'
}
if (!$env:DARTFORGE_DART_SDK -and (Test-Path 'E:\DartSDKs\3.6.2\bin\dart.exe')) {
    $env:DARTFORGE_DART_SDK = 'E:\DartSDKs\3.6.2'
}
if (!$env:DARTFORGE_DART_SDK_3_13 -and (Test-Path 'E:\DartSDKs\3.13.4\dart-sdk\bin\dart.exe')) {
    $env:DARTFORGE_DART_SDK_3_13 = 'E:\DartSDKs\3.13.4\dart-sdk'
}
