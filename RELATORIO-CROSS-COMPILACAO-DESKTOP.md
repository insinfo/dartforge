# Compilação cruzada e File Explorer — 19/10/2026

Implementação no `dartforge`, usando o File Explorer real da `dart_ui` em
`15aff70` e o SDK de fontes Dart 3.6.2. As compilações e execuções deste relatório
usam DartForge; não são uma repetição dos testes com `dart compile exe`/`dart run`.

## Implementação

- `compile-native` e `aot`: `--target` e `--target-bundle`.
- `export-target`: três runtimes estáticos, sysroot e manifesto com revisão das
  fontes do runtime. Os três sistemas exportaram o mesmo hash de fontes
  `5d6bcd9a8178d40fde4250933289fec03204d44fa6d5b66ea3c2278edcf357a8`.
- Destino imutável compartilhado pelos workers, separado do sistema do
  compilador: constantes de plataforma, ABI FFI, TLS, registradores, formatos,
  linkers e caches. Geração de objetos LLVM x64 e ARM64 registrada no processo.
- Cinco destinos: Windows x64; Linux x64/ARM64; macOS x64/ARM64. Objetos dos cinco
  tiveram seus cabeçalhos e identificadores de arquitetura conferidos em teste.
- O bundle evita usar o runtime ou bibliotecas do host no executável estrangeiro.
  JIT continua local ao processo e rejeita `--target`.

Instruções: [docs/CROSS-COMPILACAO.md](docs/CROSS-COMPILACAO.md).
Automação local: [Python](scripts/validate-native-desktop.py) e
[PowerShell](scripts/validate-native-desktop.ps1).

## Rodada de execução

Ambientes: macOS ARM64, Ubuntu 24.04 ARM64/Wayland e Windows 11 ARM64 executando
binários **x64** por compatibilidade. Windows ARM64 nativo não é um destino
implementado nesta entrega.

O programa original é iniciado com `--frames=2`, apresenta seus frames e fecha
normalmente. O smoke `context_menu_retarget_smoke.dart` usa a aplicação verdadeira
com um disco em memória: abre menu de arquivo, muda de arquivo com um único
clique secundário, abre o menu da área vazia, retorna ao arquivo e confere erros
da aplicação. Ele não injeta mouse em aplicativos de terceiros.

A matriz inicial de macOS/Linux e AOT Windows foi executada com a implementação
cruzada de `986b16d4`. O JIT Windows foi repetido após as correções de `98ee596b`.
Os compiladores finais foram montados nos três sistemas; os hashes e os smokes
adicionais de plataforma ficam nas evidências.

Resultados e hashes: [evidências da rodada](bench/results/2026-10-19-cross-desktop/README.md).
AOT e JIT do programa original passaram nos três sistemas. O smoke de menus
também passou no JIT dos três e nos executáveis AOT nativos de macOS/Linux e
cruzado de Windows.

| Sistema de execução | File Explorer AOT | File Explorer JIT | Smoke de menus AOT/JIT |
|---|---|---|---|
| macOS ARM64 | passou | passou | passou / passou |
| Ubuntu ARM64, Wayland/OpenGL | passou | passou | passou / passou |
| Windows 11 ARM64, binários x64, D3D11 | passou | passou | passou (cruzado) / passou |

Compilação cruzada real do explorador e execução no destino: **macOS → Linux
ARM64**, **macOS → Windows x64** e **Linux → macOS ARM64**, todas passaram.
Os dois primeiros caminhos também passaram no smoke de menus; os três passaram
na fixture de constantes de plataforma e efeitos colaterais. Não foi usado
`--cpu native` para gerar executáveis estrangeiros.

## Problemas encontrados durante a validação

A primeira montagem do compilador no Windows usou o destino ARM64 padrão do
Rust com LLVM x64. O link rejeitou a mistura. O compilador foi reconstruído com
`--target x86_64-pc-windows-msvc`; a arquitetura da biblioteca LLVM agora é
conferida antecipadamente contra o destino Rust. Um teste de pré-verificação
provou a recusa de uma combinação incompatível. A escolha do destino Rust do
compilador não deve ser confundida com `--target` do programa Dart.

No macOS, o linker padrão da Apple não conseguiu ler bitcode LLVM 22 das
bibliotecas estáticas do compilador. A montagem usou `ld64.lld` 22.1.8 e o SDK
macOS 26.5 disponível. O compilador Windows de validação foi montado sem LTO e
com otimização Rust reduzida; isso não desliga o `--optimize` do programa Dart.
Os tempos dessas montagens não são uma comparação de desempenho entre sistemas.

O JIT Windows inicialmente abortou com `IMAGE_REL_AMD64_ADDR32NB relocation
requires an ordered section layout`. O alocador próprio já mantinha regiões
limitadas de código/dados nos modos de mapas/rastreamento, mas não era usado pelo
perfil padrão. Ele passou a ser usado também no Windows x64 com raízes na
pilha-sombra. Todas as seções `.pdata` do objeto são registradas e removidas,
antes de liberar a memória. O teste novo materializa vários objetos simultâneos
e verifica as distâncias reais entre código e dados depois da relocação.
A rodada seguinte encontrou símbolos de constantes COFF (`__real@`/`__xmm@`)
sem materialização. A camada customizada precisa preservar também as duas
políticas de propriedade de símbolos ativadas pelo LLJIT padrão. Um bridge
Windows de duas chamadas inline aplica essas flags usando os headers da mesma
instalação LLVM, sem transferir objetos ou memória entre heaps C++. Uma regressão
materializa constantes repetidas em vários objetos.
A exigência de deslocamentos limitados consta no
[código do RuntimeDyld COFF](https://llvm.org/docs/doxygen/RuntimeDyldCOFFX86__64_8h_source.html).

A configuração de símbolos segue a implementação do
[LLJIT 22.1.8](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/lib/ExecutionEngine/Orc/LLJIT.cpp#L883).

## Verificação e limites

- Destinos/bundles: dois testes unitários; recusam revisão, destino e caminhos
  incompatíveis. CLI: regressão de opções inválidas e do limite do JIT local.
- JIT Windows: 14 testes dirigidos passaram, incluindo os testes ignorados de
  execução/erros/unload e as duas novas regressões; dois testes diferenciais
  maiores ficaram fora dessa rodada dirigida.
- Objetos: teste passou para ELF x64/ARM64, Mach-O x64/ARM64 e COFF x64.
- Doctests do módulo de destino: três passaram. Documentação do workspace foi
  gerada com sucesso.
- Backend nativo: 763 passaram, quatro falharam, nove ignorados. As quatro
  falhas são exatamente as registradas em `/tmp/dartforge-native-final.log`
  antes desta implementação: certificado de clamp, layout ARC de notifier,
  certificado de enum e especialização do kernel de cor. Não foram convertidas
  em sucesso nem ocultadas.
- `cargo fmt --all -- --check` não está limpo no workspace. Clippy com
  `-D warnings` parou em seis problemas preexistentes de `crates/runtime`.
  A rodada completa de testes do workspace parou na ligação LLVM 22 versus o
  linker padrão da Apple; as verificações dirigidas usaram a configuração
  compatível descrita acima.

Os recursos da aplicação continuam sendo responsabilidade de seu empacotamento.
As execuções estrangeiras apontam `DART_UI_FONTS` para as fontes do destino.
O macOS usa o host AppKit já existente da `dart_ui`; esta rodada não certifica
AppKit direto na thread principal do JIT. Não houve certificação de todos os
recursos do explorador ou de execução prolongada, e nenhum agente periódico
foi instalado.
