# Compilador Nativo (LLVM) — Trilha Nova

O que o backend nativo **faz hoje**, lido no código. O que falta e a ordem em
que entra estão em `docs/NATIVO-PLANO.md` (§6 o contrato R/E/N/G; §7 a rodada 2:
decisões, passos P0–P9 e o mapa de módulos com os donos de cada arquivo). Onde
este documento diz "ainda não", o construto vira diagnóstico de compilação.

---

## 1. Arquitetura

O compilador nativo consome os artefatos semânticos da trilha nova:

- `Program` (`crates/elements/src/model.rs`);
- `OutlineTypes` (`crates/types/src/resolve.rs`);
- `BodyTypes` (`crates/types/src/resolved.rs`);
- `TypeTable` e `CoreTypes` (`crates/types/src/table.rs`).

Etapas (`crates/emit_native/src/lib.rs`, `emitir_ir` e `compilar`):

1. **Carregamento e inferência.** O programa e o SDK (seção `vm` do
   `libraries.json`, com os patches e a sobreposição `sdk_nativo/`) viram um
   `Program`; os corpos **do programa** são inferidos. O SDK da fonte é o
   padrão (`sdk_modulo::sdk_da_fonte_pedido`,
   `crates/emit_native/src/sdk_modulo.rs:121-127`): as
   `BIBLIOTECAS_DA_FONTE` (`sdk_modulo.rs:25`) são compiladas uma vez por
   conteúdo num módulo próprio (a biblioteca `dfsdk_<chave>` de §1.1, ou
   bitcode ThinLTO na produção), com ids de classe fixados pela tabela do SDK
   compilado (`Context::com_sdk_da_fonte_e_ids`, `crates/emit_native/src/lib.rs:233-238`).
   É o único modo desde o espaço unificado (o casamento por nome saiu, §3).
2. **Lowering para a HIR própria** (`crates/emit_native/src/hir.rs`,
   `crates/emit_native/src/lower/`). Chamada a função do usuário é direta; a
   método de instância do usuário é um `switch` sobre a classe dinâmica
   (`dartforge_value_class`) entre as implementações (`lower/membros.rs`). Não
   há vtable; `CallInterface`/`CallDynamic` continuam sem emissão (o
   verificador da HIR as recusa, `lower/verificador.rs:234-241`), e o despacho
   por seletor (`CallSeletor`, tabela de métodos da classe com cache por
   ponto de chamada, `hir.rs:405-417`) atende o receptor sem tipo e o código
   do SDK. Além de `??`, `??=`, `?.`, `for-in`,
   `try`/`catch`/`finally` e `assert`, são baixados: closures e tear-offs
   (`lower/closures.rs`, `lower/captura.rs`), cascatas (`lower/cascata.rs`),
   `switch` e padrões (`lower/comandos.rs`, `lower/padroes.rs`),
   `async`/`await`, `sync*` e `async*` (`lower/async_sm.rs`), extensões
   (`lower/extensoes.rs`) e `super.m()` (`lower/chamadas.rs`). O que ainda
   falta aparece no relatório do harness (§4).

   > **Histórico (até 2026-09-27).** Este passo dizia que closures,
   > cascatas, `switch`, padrões, `async`/`await`, geradores, extensões e
   > `super.m()` **não** eram baixados. Era o estado da rodada 1; entraram em
   > P1–P4 e P6 (`docs/NATIVO-PLANO.md` §7.5 e §7.7).
3. **Construto não suportado** no código **do programa** vira diagnóstico
   com posição (N1). Se o módulo tem algum, `emitir_ir` e `compilar`
   devolvem `Err` com **todos** os diagnósticos (a primeira linha é a do
   primeiro, sem posição, para agrupar), e nenhum IR é emitido. No código
   **do SDK da fonte** o mesmo construto vira `UnsupportedError` em tempo de
   execução, com o mesmo texto (`FnBuilder::nao_suportado`,
   `crates/emit_native/src/lower/fn_builder.rs:316-343`); é o caso dos
   natives pendentes (`docs/NATIVOS-PENDENTES.md`).
4. **Emissão de LLVM IR** textual (`crates/emit_native/src/llvm/`), ponteiros
   opacos.
5. **Clang e ligação** (`crates/emit_native/src/driver.rs`), com cache de
   objeto por hash do IR (`cache_objeto.rs`) e o runtime Rust compilado uma vez
   por conteúdo (`cache.rs`).

### 1.1 Sistemas

Windows (COFF), Linux (ELF) e macOS (Mach-O); o alvo é sempre o do
hospedeiro, e tudo o que muda entre eles está em
`crates/emit_native/src/alvo.rs`:

| | Windows | Linux | macOS |
| --- | --- | --- | --- |
| cabeçalho do IR | `x86_64-pc-windows-msvc` | `x86_64-unknown-linux-gnu` | nenhum (o do hospedeiro) |
| `comdat` | sim | sim | não (`linkonce_odr` já é fraco) |
| SDK da fonte (desenvolvimento) | `dfsdk_<chave>.dll` + `.lib` de importação, `/DEF` | `libdfsdk_<chave>.so`, `-soname`, `rpath=$ORIGIN` | `libdfsdk_<chave>.dylib`, `@rpath`, `rpath=@executable_path` |
| produção (ThinLTO) | `lld-link`, `/OPT:REF`, `/OPT:SAFEICF` | `ld.lld`, `--gc-sections`, `--icf=safe`, sem símbolos | `ld64.lld`, `-dead_strip`, sem símbolos (sem ICF) |
| ligador (todo perfil) | `lld-link` direto (`ligador_windows.rs`) | `ld.lld` direto (`ligador.rs`) | `ld64.lld` direto (`ligador_macos.rs`) |
| o que vem do sistema | nada: bibliotecas de importação e CRT mínima geradas pelo dartforge | glibc e libgcc copiadas (sysroot) | nada: `.tbd` gerados pelo dartforge |

O IR e as bandeiras no Windows são os de antes do porte, então as chaves de
cache e os resumos de determinismo não mudaram. O mesmo IR vai ao JIT
(`docs/JIT.md`, «Linux e macOS»).

**O executável de produção** (`aot --optimize`) difere do de desenvolvimento
em três pontos, todos para tamanho (docs/PLANO-TAMANHO-DESEMPENHO.md §1):

* **O runtime é compilado com `panic=abort`.** É a variante
  `dartforge_rtprod_*` da `staticlib`. A do executável sem o SDK da fonte
  (`dartforge_runtime_*`) também aborta. A da DLL de desenvolvimento, que o
  JIT carrega, continua desenrolando.
  * Um pânico no código que o Dart chama já encerrava o processo: ele chega a
    uma fronteira `extern "C"` (as entradas do runtime, o `main` C, o código
    Dart que a thread de um isolado roda), onde o Rust aborta.
  * O que muda é o pânico no Rust puro de uma thread auxiliar: a preparação
    de um isolado e as threads do `dart:io`. Antes ele matava só a thread e
    o resto podia ficar esperando por ela; agora encerra o processo, com a
    mensagem do pânico.
  * Nenhum `catch_unwind` nem `join` do runtime conta com o desenrolamento.
* **O descritor `@df.area` de cada módulo é `[chave, -n]`**, sem os hashes
  dos nomes dos slots. Esses hashes só servem à recarga, e a produção nunca
  recarrega.
* **As funções idênticas são juntadas pelo ICF seguro**, e as do SDK levam
  `optsize`. O ICF seguro só junta as funções cujo endereço ninguém toma,
  então o `==` de tear-offs de funções de corpo idêntico continua `false`
  (corpus/nativo 80).

### 1.2 Ligação sem o toolchain de C do sistema (N15, N16)

Nenhum sistema usa mais o driver do Clang para ligar: o `driver.rs` e o
`sdk_modulo.rs` chamam o ligador do LLVM (o da distribuição, `lib/llvm/bin`,
ou o ao lado do Clang numa árvore de desenvolvimento) com tudo explícito na
linha de comando. O que o sistema exigiria vem do **sysroot de ligação**,
`lib/sysroot/<triple>/` na distribuição (`dartforge empacotar`); numa árvore
de desenvolvimento, do sistema (Linux, macOS) ou do cache nativo (Windows).

**Windows (N15): sem Visual Studio (MSVC) nem Windows SDK.** Duas saídas
foram avaliadas:

* (b) o alvo `x86_64-pc-windows-gnu` com os objetos MinGW que o Rust traz:
  **descartada**. Mudaria o triple do IR (as chaves de cache, os resumos de
  determinismo e o JIT, que exige o triple do processo `-msvc`), o
  `long double`, o SEH do pânico do Rust e o CodeView (J05) viraria DWARF.
  E já não resolve: o `rustlib/x86_64-pc-windows-gnu/lib/self-contained` do
  Rust 1.98 só tem `crt2.o` e `dllcrt2.o` (conferido) — as bibliotecas de
  importação do MinGW saíram de lá quando o `std` passou a `raw-dylib`.
* (a) **adotada**: o mesmo alvo `x86_64-pc-windows-msvc`, os mesmos objetos
  (o IR não mudou), e o `lld-link` com `/nodefaultlib /lldignoreenv` e só o
  que o dartforge gera (`crates/emit_native/src/ligador_windows.rs`):
  * **bibliotecas de importação** escritas em Rust: arquivos `ar` com
    membros COFF *short import* (`IMPORT_OBJECT_HEADER`, PE/COFF §8), um por
    função, a partir de listas (`IMPORTACOES`) com só os nomes que o runtime
    (Rust, `ring`, zlib), o SDK e o código gerado usam — `kernel32`,
    `ntdll`, `ws2_32`, `iphlpapi`, `advapi32`, `userenv`, `bcrypt`,
    `crypt32` e a CRT universal `ucrtbase.dll` (parte do Windows 10 e
    posteriores). Nada do MSVC nem do Windows SDK é copiado: as licenças
    deles não permitem redistribuir as `.lib`, e uma lista de nomes de
    funções é um fato de interface. O teste `importacoes_existem_no_sistema`
    confere cada nome nas DLLs do `System32` do runner;
  * a **CRT mínima**, em LLVM IR compilado pelo mesmo gerador: o diretório
    de TLS (`_tls_used`, `_tls_index` e os marcadores `.tls`/`.CRT$XL*`,
    entre os quais o Rust registra as *callbacks* de TLS), os
    inicializadores `.CRT$XI*`/`.CRT$XC*`, as entradas `mainCRTStartup` e
    `_DllMainCRTStartup`, o cookie do `/GS` (`__security_cookie`,
    `__security_check_cookie`, `__GSHandlerCheck`: o C do `ring` e do zlib
    vem do `cl.exe`), `__chkstk`, `_fltused`, `atexit` (global no
    executável, pelo `_crt_atexit`; por DLL, percorrida na descarga) e a
    vtable de `type_info` que o descritor de tipo do pânico do Rust aponta.
  O SEH e o C++ EH continuam os do sistema (`__C_specific_handler`,
  `__CxxFrameHandler3` e `_CxxThrowException` do `ucrtbase`), o CodeView e o
  PDB (`/debug`, J05) são os do `lld-link`, e o executável deixa de depender
  do `vcruntime140.dll`, que não faz parte do Windows. A DLL do SDK da fonte
  é ligada igual (`/dll /def:`, a biblioteca de importação sai ao lado).
  O teste `liga_e_executa_programa_c` liga (em qualquer sistema com Clang e
  `lld-link`) um executável e uma DLL em C com TLS, inicializador,
  `__chkstk`, `libm` e `atexit`, e os executa no Windows ou no Wine.

**macOS (N16): sem Xcode nem Command Line Tools.** O `ld64.lld` direto
(`ligador_macos.rs`) com `-syslibroot` no sysroot, `-platform_version` (o
mínimo do `rustc` para o alvo, também como versão do SDK) e
`-lSystem -framework CoreFoundation -framework Security`. Os `.tbd` (TAPI v4,
texto: caminho de instalação e nomes exportados) são **gerados pelo
dartforge** (`BIBLIOTECAS_DO_SISTEMA`, por arquitetura: o x86-64 usa as
variantes `$INODE64`/`$NOCANCEL`), como as bibliotecas de importação do
Windows: nada do SDK da Apple é copiado, porque a licença dele (Xcode and
Apple SDKs Agreement) não autoriza redistribuí-lo. O `dartforge empacotar`
os escreve em `lib/sysroot/<triple>`; numa árvore de desenvolvimento vão
para o cache nativo (`ligacao-macos/<arch>-<impressão>`), então toda ligação
no macOS, com ou sem Xcode, usa os mesmos arquivos. A `libSystem` basta para
tudo o que ela reexporta (`libsystem_c`, `libdyld`, `libunwind`,
`libdispatch`, `libcommonCrypto`, `libsystem_m`…): o `dyld` acha cada nome
pelas reexportações ao carregar. A lista saiu do `nm` das duas `staticlib`
do runtime (compiladas para `aarch64-` e `x86_64-apple-darwin`), mais a
`libm` e as chamadas que o LLVM emite sozinho (`___sincos_stret`,
`_memset_pattern16`, `___chkstk_darwin`). No macOS,
`runtime_so_usa_simbolos_da_lista` confere que o runtime não pede nada fora
dela e `simbolos_existem_no_sdk`, que cada nome existe no `.tbd` do SDK.

**A prova** é o workflow `sem-toolchain.yml`: compila o dartforge, monta a
distribuição, **esconde** o toolchain (no Windows renomeia as pastas do
Visual Studio e dos Windows Kits e roda com `PATH` mínimo, sem vcvars,
`LIB` nem `INCLUDE`; no macOS move o Xcode e as Command Line Tools e roda
com `DEVELOPER_DIR` vazio, onde o `xcrun` falha), confere que ele sumiu e
então liga e executa o olá mundo (desenvolvimento, produção e depuração) e
o corpus nativo contra a VM pela distribuição (`DARTFORGE_HOME`).

---

## 2. Modelo de objetos (runtime, `crates/runtime`)

O runtime é Rust, com heap preciso e **um espaço de objetos só**
(docs/NATIVO-ESPACO-UNIFICADO.md): todo valor do runtime — objeto do
programa, string, caixa, lista, closure, contexto, lista tipada — é um bloco
com o mesmo cabeçalho de 16 bytes e uma classe (`cid`) no cabeçalho. Um
`Ref` do código gerado é um `i64`: `0` é null; um valor **ímpar** é um `int`
pequeno etiquetado, o `Smi` (R10, NATIVO-PLANO §6.2), que não aloca e que o
coletor nunca segue; `h & 7 == 2` é um objeto, com o bloco em `h - 2`; o resto
é inválido. O contrato numérico (deslocamentos, cids, classes de tamanho,
formatos) está em `crates/runtime/src/layout.rs`, usado pelo runtime e pelo
emissor (`dartforge_runtime::layout`). O código fonte do runtime são os
fragmentos de `crates/runtime/src/` concatenados na ordem de `FRAGMENTOS`
(`crates/runtime/build.rs`) — o mesmo texto para o AOT (`rustc` avulso) e para
o JIT (módulo `dartforge_runtime::abi`) —, mais os módulos `heap`, `layout`,
`espaco` e as vistas por tema (`textos`, `caixas`, `listas`, `tipadas`).

- **Cabeçalho** (`layout::Cabecalho`): estado de coleta (com `PERMANENTE`
  para os objetos estáticos da imagem), `flags` (o **formato do corpo** —
  `INSTANCIA`, `BRUTO` ou `REFS` —, cartões, anexo nativo, forma compacta de
  lista, memória externa), `n`, o `cid`, o `mapa` (os bits de referência dos
  32 primeiros campos de uma instância; nas strings, o hash) e o metadado de
  RTI (`id + 1`). O coletor percorre o corpo pelo formato, sem olhar a classe:
  `INSTANCIA` pelos bits do mapa, `REFS` toda palavra par e não nula depois da
  palavra 0, `BRUTO` nada.
- **Classes fixas** (`layout::cid`, §2.4 da especificação): 1–65 são as
  classes que o runtime conhece (`Null` 1, `_Smi` 2, `_Mint` 3, `_Double` 4,
  `bool` 5, `_OneByteString` 6, `_TwoByteString` 7, `_List` 8,
  `_ImmutableList` 9, `_GrowableList` 10, `_Closure` 11, `_Record` 12, o
  contexto e a célula de closure 13/14, o acumulador do `StringBuffer` e o
  programa de `RegExp` 15/16, `_SendPort`/`_Capability` 17/18, SIMD 19–21 e as
  listas tipadas e visões 22–65); as classes do programa e do resto do SDK
  começam em 128. O `cid` de um objeto nunca muda depois de publicado: `_List`
  é a lista fixa, `_ImmutableList` a imutável e `_GrowableList` a expansível,
  escolhidas na alocação. O código gerado lê a classe em linha
  (`@df.classe`: `0` → 1, ímpar → 2, senão o `cid` do cabeçalho), e o módulo
  confere na partida que a tabela de classes do runtime é a dele
  (`dartforge_registrar_cids`, o marcador de ABI).
- **Objeto de classe do programa:** `INSTANCIA`, cada campo uma palavra (bits
  crus ou `Ref`) com o bit de referência no mapa (E1). O código gerado aloca
  em linha pela TLAB do isolado (`@df.alocar`, qualquer bloco de até 16
  palavras), lê e grava campos em `h + 14` sem chamar o runtime e, depois de
  gravar um `Ref`, passa pela barreira de escrita (`@df.barreira`: um pai
  velho que recebe um filho jovem é lembrado; a lista grande marca o cartão do
  elemento). As classes de erro do runtime têm ids fixos 1000–1012.
- **`int`:** `i64` com estouro modular, como a VM. **`double`:** `f64`.
  **`bool`:** `i1` no IR, `u8` na fronteira com o runtime. Numa posição `Ref`
  (`Object?`, `dynamic`) o `int` vira `Smi` quando cabe em 63 bits e `_Mint`
  (bloco `BRUTO` de uma palavra) quando não cabe; `double` vira `_Double`;
  `bool` é uma das duas caixas **estáticas** do runtime (`dartforge_falso`,
  `dartforge_verdadeiro`). `identical` compara o handle, ou o valor de dois
  `_Mint`/`_Double` (`@df.identico`, como `Instance::IsIdenticalTo` da VM).
- **`String`:** `_OneByteString` (toda unidade ≤ 0xFF, um byte cada) ou
  `_TwoByteString` (dois bytes), blocos `BRUTO` com o comprimento em `h + 14` e
  as unidades a partir de `h + 22`, escolhidos pelo conteúdo e canônicos — um
  pedaço Latin-1 de um texto de dois bytes volta a um byte. O `hashCode` é o
  `StringHasher` da VM (`layout::hash_de_texto`), guardado no cabeçalho na
  primeira consulta. `length`, `codeUnitAt`, a alocação e a gravação de
  unidades do SDK (`allocateOneByteString`, `writeIntoOneByteString`) são em
  linha (`@df.texto_*`). Um surrogate solto é uma unidade como outra; só o
  `print` o troca por U+FFFD, como a VM (`Utf8::Encode`). **Literais:** no AOT,
  cada literal é um objeto estático do módulo (`@df.s.<chave>`,
  `linkonce_odr` numa seção própria, estado `PERMANENTE`), sem chamada no ponto
  de uso, e o mesmo literal é o mesmo objeto entre bibliotecas; no JIT (cuja
  memória de uma geração é liberada), o literal é internado no heap
  (`dartforge_string_new`, cache no ponto de uso). O acumulador do
  `StringBuffer` e o programa compilado de um `RegExp` são blocos com **anexo
  nativo**, soltos pelo coletor quando o dono morre.
- **Listas:** `_List`/`_ImmutableList` são `REFS` (o comprimento e os
  elementos `Ref`) ou, quando o `E` reificado é exatamente `int`, `double` ou
  `bool`, **compactas** (`BRUTO` com a forma em `flags`: os 8 bytes de cada
  elemento, sem caixa); a forma não é observável e troca no lugar quando o
  tipo é gravado (`rti_definir`). `_GrowableList` é `INSTANCIA` de dois campos:
  o comprimento e o armazenamento (uma `_List`), com o crescimento da VM
  (`(capacidade * 2) | 3`). Uma lista geral acima de 2014 elementos tem
  **cartões** (um bit por 32 elementos), e a coleta menor percorre só os
  cartões sujos de uma lista velha. `Map`/`Set` são o `_Map`/`_Set` da fonte do
  SDK (o `_data` uma `_List`, o `_index` uma `_Uint32List`).
- **Listas tipadas e SIMD:** a lista interna é `BRUTO` com o comprimento em
  `h + 14` e o endereço dos dados em `h + 22` (apontando para dentro do próprio
  bloco, ou para a memória de fora de um `asTypedList`, `EXTERNO`); a visão é
  `INSTANCIA` com o comprimento e os dados nos mesmos lugares, a base e o
  deslocamento. O código gerado lê `len` e `dados` sem saber qual das três é.
  O coletor não move, então os dados têm endereço fixo enquanto o objeto
  vive (FFI, TLS e E/S síncrona usam o ponteiro direto).
- **Closures:** `_Closure` (`INSTANCIA`: o código, o contexto, o corpo tipado e
  a ABI), `_Contexto` (as capturas) e `_Celula` (a variável capturada mutável),
  alocados em linha pelo lowering (`lower/closures.rs`, `lower/captura.rs`).
  Uma captura é lida na representação com que foi gravada — o verificador da
  HIR (`lower/verificador.rs`) recusa a divergência. O record posicional é
  `_Record` (`REFS`); o nomeado, um objeto da classe da forma.
- **Mensagens entre isolados** (`portas.rs`): o grafo copia os blocos pelo
  formato — campos com o bit de referência, palavras `REFS`, corpos `BRUTO`
  crus —, refaz no destino o que aponta para dentro de si ou para fora do heap
  (lista tipada, visão, anexo) e passa os objetos estáticos pela identidade.
- **Exceções:** modelo por valor — exceção pendente no runtime e verificação
  depois de cada chamada (NATIVO-PLANO §1), com `try`/`on T`/`catch (e, s)`/
  `finally` e `rethrow`.
- **Estado:** heap, nomes de classe e exceção pendente em `thread_local!` do
  runtime; os globais Dart (`@dfg_<id>` e a bandeira `$ok`) são slots da
  **área de globais do isolado** — cada isolado é uma thread com a sua área,
  como a *field table* da VM (`crates/emit_native/src/llvm/mod.rs`; runtime
  em `crates/runtime/src/gc_raizes.rs`).

> **Histórico (até 2026-09-30).** Até o espaço unificado havia dois heaps no
> mesmo isolado: o espaço de objetos (só os objetos das classes do programa e
> do SDK compilado) e uma **tabela de slots** (`Heap::slots`, um `enum Value`
> por slot, handles múltiplos de 4) com strings, caixas, listas, closures,
> listas tipadas e, sem o SDK da fonte, mapas e conjuntos, cada um com o
> conteúdo num `Vec` do `malloc` e um segundo coletor. Os elementos das
> listas gerais eram `TaggedValue` de 16 bytes, e o código gerado lia listas,
> strings e closures por cabeçalhos de endereço fixo (`CabecalhoDeLista`,
> `CabecalhoTipado`, `CabecalhoDeClosure`) e por externs de acesso. Tudo isso
> saiu (docs/NATIVO-ESPACO-UNIFICADO.md §2.17).

---

## 3. Membros do SDK

Os membros do SDK vêm da **fonte do SDK 3.6.2** com a sobreposição
`sdk_nativo/` (`sdk_nativo/libraries.json`) e os natives em Rust da tabela
`crates/emit_native/src/nativos.rs`; o que cada native pendente significa
para a API pública está em `docs/NATIVOS-PENDENTES.md`. É o único modo: um
membro do SDK sem elemento útil no ponto da chamada (receptor `dynamic`,
`Object`, corpo de closure sem tipo) vai pela classe dinâmica do receptor,
por seletor (`lower/chamadas.rs`, `metodo_por_seletor`;
`lower/expressoes.rs`, `propriedade_por_seletor`), com o `noSuchMethod` do
receptor na falta.

> **Histórico (até 2026-09-30).** O runtime escrito à mão que casava membros
> **pelo nome** (`lower/sdk_por_nome.rs`: `length`, `add`, `substring`,
> `join`, `Exception(…)`, `StringBuffer()`…), congelado desde a rodada 2 e
> usado só com `DARTFORGE_SDK_DA_FONTE=0`, saiu com o espaço unificado, com
> o despacho pelo nome em mundo fechado (`lower/despacho.rs`) e os externs
> `dartforge_list_*`/`map_*`/`set_*`/`string_*` que só ele emitia
> (docs/NATIVO-ESPACO-UNIFICADO.md §3.7, §4.7). Até 2026-09-27 esta seção
> dizia que ele seria apagado em P5; P5c/P5d o deixaram atrás da variável
> de ambiente.

---

## 4. Medição

O placar é o do harness (`crates/diferencial`), no CI:
`pwsh scripts/ci.ps1 -Frente <frente> -Acompanhar` e
`pwsh scripts/ci.ps1 -Placar <id>`. O relatório nativo termina com duas
seções: as falhas agrupadas pela primeira linha do stderr, e **"construtos
(todos os diagnósticos)"** — cada construto não suportado com o número de
programas que o usam e a lista dos programas bloqueados **só** por ele.

---

## 5. Estado de `late`

Globais e campos estáticos usam a bandeira de inicialização do getter;
campos de instância sem inicializador usam uma marca por objeto e índice,
purgada pelo GC. A marca não depende dos bits do valor, pois `0`, `false` e
`null` podem ser atribuições válidas. Locais não capturados usam uma marca
na pilha; seu inicializador roda na primeira leitura e uma escrita anterior
cancela essa avaliação. Os erros usam `LateError` do SDK da fonte.

Locais `late` capturados sem inicializador compartilham o estado pela `Cell`
da variável; uma entrada de índice `-1` na tabela lateral acompanha a vida
do handle e é purgada pelo GC. A captura de inicializador preguiçoso ainda
precisa transportar o ambiente da declaração. Campos `late` com inicializador
usam um getter próprio por campo. A tabela lateral distingue o valor já
inicializado (inclusive `null`) da avaliação em curso, por objeto e índice;
a leitura reentrante produz `StackOverflowError` do SDK sem recursão no
lowering. O índice `-(campo+2)` reserva a marca transitória sem colidir com
`-1` dos locais capturados. O estado transitório também é purgado pelo GC.

Um global com inicializador distingue três estados (`0` pendente, `2` em
avaliação, `1` pronto). A leitura reentrante constrói `StackOverflowError`
do SDK, que é o erro capturável observado na VM, e uma exceção durante a
avaliação restaura o estado pendente para a próxima leitura.
