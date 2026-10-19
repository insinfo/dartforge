# Compilação cruzada nativa

`aot` e `compile-native` aceitam `--target <destino>` e
`--target-bundle <diretório>`. O compilador roda no host, emite código para o
destino e usa o linker LLVM executável no host. A compilação não é delegada
para a VM. A VM serve para exportar os insumos de ligação e executar o resultado.

Destinos implementados:

| Nome | Triple | Formato |
|---|---|---|
| windows-x64 | x86_64-pc-windows-msvc | PE/COFF |
| linux-x64 | x86_64-unknown-linux-gnu | ELF/glibc |
| linux-arm64 | aarch64-unknown-linux-gnu | ELF/glibc |
| macos-x64 | x86_64-apple-darwin | Mach-O |
| macos-arm64 | aarch64-apple-darwin | Mach-O |

Para construir o próprio compilador na VM Windows ARM64 com LLVM x64, selecione
explicitamente o destino Rust: `cargo build --release -p dartforge-cli --features
jit --target x86_64-pc-windows-msvc`. Não misture uma biblioteca LLVM x64 com um
executável Rust ARM64; o build agora verifica essa arquitetura antecipadamente.
O JIT Windows requer a instalação LLVM completa (headers C++, `clang-cl` e
`llvm-lib`) e os headers C++ do MSVC. Um bridge pequeno preserva as políticas
COFF do LLJIT quando o alocador delimitado de código/dados é instalado.

### Construir o compilador no macOS com LLVM estático

Se o linker Apple recusar bitcode LLVM 22 com `Unknown attribute kind`, use o
linker da mesma instalação LLVM para montar o compilador Rust:

```sh
export DARTFORGE_LLVM_DIR=/caminho/para/LLVM-22.1.8
python3 scripts/cargo-macos-llvm.py build --locked --release -p dartforge-cli --features jit
python3 scripts/cargo-macos-llvm.py test --locked -p dartforge-llvm
```

O comando testa uma ligação C mínima antes de executar o Cargo. Sem `SDKROOT`,
tenta o SDK ativo e os SDKs instalados ao lado dele até encontrar um compatível.
Um `SDKROOT` explícito é respeitado: se incompatível, o comando falha sem trocá-lo.
O Python da Apple pode preencher essa variável automaticamente; nesse caso,
informe o SDK desejado ao iniciar o comando. Nesta máquina, LLVM 22 aceita
`SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk`; os stubs do SDK
27 instalado foram recusados. Os caminhos dependem da instalação local.

As flags Rust existentes são preservadas e a configuração só vale para o
processo filho. Não altera preferências do Xcode, o SDK global ou o Cargo do
usuário. Esse comando monta o compilador no macOS; a escolha do destino do
programa Dart continua sendo feita por `--target` e `--target-bundle`.

Windows ARM64 executa o destino x64 por compatibilidade; não há destino nativo
Windows ARM64 nesta implementação. Os triples da tabela também são aceitos.
Sem `--target`, o comportamento continua sendo compilar para o host.

## Preparar um destino

No sistema de destino, com um DartForge compilado das mesmas fontes:

```sh
dartforge export-target /path/to/linux-arm64-bundle
```

O diretório deve estar vazio. Ele recebe `target.json`, os três runtimes estáticos
e `sysroot/`. O manifesto identifica o destino e o hash das fontes do runtime e
`Cargo.lock`, normalizando finais de linha. O consumidor recusa revisão ou alvo
incompatíveis. Copie esse diretório para o host compilador. Não é necessário
copiar o executável do compilador ou o linker da VM.

No host, com LLVM 22.1.8 e fontes do SDK Dart 3.6.2:

```sh
dartforge compile-native main.dart --optimize \
  --target linux-arm64 --target-bundle /path/to/linux-arm64-bundle \
  --sdk /path/to/dart-sdk/lib -o app-linux-arm64

dartforge compile-native main.dart --optimize \
  --target windows-x64 --target-bundle /path/to/windows-x64-bundle \
  --sdk /path/to/dart-sdk/lib -o app-windows.exe
```

O pacote Windows contém bibliotecas de importação/CRT mínimo gerados pelo projeto.
O pacote macOS contém os stubs `.tbd` gerados pelo projeto. O Linux contém os
objetos de inicialização e bibliotecas glibc/libgcc do sistema de exportação; o
executável resultante precisa de um ambiente compatível com essas versões.
Arquivos referenciados pelo próprio programa (fontes, imagens, recursos e hosts
auxiliares da `dart_ui`) continuam sendo responsabilidade do empacotamento da
aplicação. O bundle serve à ligação, não é um instalador do aplicativo.

## JIT e limites da validação

JIT executa código no próprio processo: `dartforge run` e `reload` não aceitam
`--target`. Execute o compilador nativo de cada sistema para testar JIT nele.
`--cpu native` é recusado numa compilação cruzada, para não copiar instruções do
host para outro processador.

O alvo é imutável por processo e compartilhado pelos workers. A seleção afeta
constantes de plataforma, ABI C, exceções/TLS, registradores, seções, cabeçalhos
LLVM, bibliotecas e caches. O cache de um alvo estrangeiro fica separado do
cache nativo. A presença de um destino na tabela não equivale à validação de
todo programa Dart nesse destino; conferir o relatório da rodada real.
