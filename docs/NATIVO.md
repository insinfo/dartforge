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
   O casamento por nome de §3 só volta com `DARTFORGE_SDK_DA_FONTE=0`.
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
| produção (ThinLTO) | `lld-link`, `/OPT:REF` | `ld.lld`, `--gc-sections`, sem símbolos | `ld64.lld`, `-dead_strip`, sem símbolos |
| ligador (todo perfil) | `lld-link` direto (`ligador_windows.rs`) | `ld.lld` direto (`ligador.rs`) | `ld64.lld` direto (`ligador_macos.rs`) |
| o que vem do sistema | nada: bibliotecas de importação e CRT mínima geradas pelo dartforge | glibc e libgcc copiadas (sysroot) | `.tbd` do SDK copiados (sysroot) |

O IR e as bandeiras no Windows são os de antes do porte, então as chaves de
cache e os resumos de determinismo não mudaram. O mesmo IR vai ao JIT
(`docs/JIT.md`, «Linux e macOS»).

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
mínimo do `rustc` para o alvo e a versão do SDK de onde os `.tbd` vieram) e
`-lSystem -liconv -framework CoreFoundation -framework Security`. Os `.tbd`
(texto: nomes de símbolos e caminho de instalação) são copiados pelo
`dartforge empacotar` do SDK da máquina que monta a distribuição, com as
bibliotecas que eles reexportam sem trazer no mesmo arquivo, como o sysroot
do Linux. **Ressalva legal (aberta):** os `.tbd` são arquivos do SDK da
Apple, cuja licença (Xcode and Apple SDKs Agreement) não autoriza
redistribuí-los; para uma distribuição pública, o caminho limpo é o do
Windows — gerar os `.tbd` a partir da lista de símbolos que o runtime usa
(um `.tbd` é YAML com nomes), sem copiar nada do SDK.

**A prova** é o workflow `sem-toolchain.yml`: compila o dartforge, monta a
distribuição, **esconde** o toolchain (no Windows renomeia as pastas do
Visual Studio e dos Windows Kits e roda com `PATH` mínimo, sem vcvars,
`LIB` nem `INCLUDE`; no macOS move o Xcode e as Command Line Tools e roda
com `DEVELOPER_DIR` vazio, onde o `xcrun` falha), confere que ele sumiu e
então liga e executa o olá mundo (desenvolvimento, produção e depuração) e
o corpus nativo contra a VM pela distribuição (`DARTFORGE_HOME`).

---

## 2. Modelo de objetos (runtime, `crates/runtime`)

O runtime é Rust, com heap preciso por **handles** (`heap.rs`): um `Ref` do
código gerado é um `i64` — `0` é null, um valor **par** indexa a tabela de
handles e um valor **ímpar** é um `int` pequeno etiquetado, o `Smi` (R10,
NATIVO-PLANO §6.2), que não aloca e que o coletor nunca segue. O código
fonte do runtime são os fragmentos `nucleo`, `gc_raizes`, `excecoes`, `saida`,
`strings`, `colecoes` e `closures` de `crates/runtime/src/`, concatenados na
ordem de `FRAGMENTOS` (`crates/runtime/build.rs`) — o mesmo texto para o AOT
(`rustc` avulso) e para o JIT (módulo `dartforge_runtime::abi`).

- **Objeto de classe do usuário:** `Value::Object { class_id, fields }`, cada
  campo `(bits: i64, is_ref: bool)` — a marcação `is_ref` é o que o coletor
  segue (E1). O `class_id` hoje é a posição da classe na carga + 1, e as
  classes de erro do SDK têm ids fixos 1000–1012 (`lower/mod.rs`); ids estáveis
  por `DeclId` entram em P2.
- **`int`:** `i64` com estouro modular, como a VM. **`double`:** `f64`.
  **`bool`:** `i1` no IR, `u8` na fronteira com o runtime. Numa posição `Ref`
  (`Object?`, `dynamic`) o `int` vira `Smi` quando cabe em 63 bits (sem
  alocação) e `_Mint` no heap quando não cabe; `double` vira caixa no heap e
  `bool` um de dois singletons (R3/R10). As coleções guardam o escalar com a
  tag, nunca a caixa nem o `Smi` (R8).
- **`String`:** unidades de código UTF-16 na forma da VM (`Texto`,
  `heap.rs`; NATIVO-PLANO §7, decisão 5): `_OneByteString` (toda unidade ≤
  0xFF, um byte cada, Latin-1) ou `_TwoByteString` (dois bytes), escolhido
  pelo conteúdo e canônico — um pedaço Latin-1 de um texto de dois bytes
  volta a um byte. `length`, índices, `codeUnitAt`, `substring`, busca,
  `split`, `padLeft` e comparação são por unidade, em O(1) por acesso; um
  surrogate solto é uma unidade como outra (`runes` o devolve como ele
  mesmo). Só o `print` troca o surrogate solto por U+FFFD, como a VM
  (`Utf8::Encode`, `runtime/vm/unicode.cc`). As constantes chegam do IR em
  UTF-8 e o runtime aceita WTF-8 (`dartforge_string_new`); `toString` e
  interpolação montam o texto por unidades (`TextoMut`), sem passar por
  `String` do Rust. `StringBuffer` guarda as unidades.
- **Listas, mapas, conjuntos:** valores do runtime com slots etiquetados
  `(bits, tag)`; os membros são externs `dartforge_list_*`, `dartforge_map_*`,
  `dartforge_set_*`.
- **Closures:** `Value::Closure`/`Environment`/`Cell` do runtime, produzidas
  pelo lowering (`lower/closures.rs`): o código da closure é o endereço da
  entrada uniforme (`ptrtoint`, `crates/emit_native/src/llvm/mod.rs`, emissão
  de `AllocClosure`), o ambiente guarda as variáveis livres ou as células
  delas (`lower/captura.rs`). Teste: `corpus/nativo/28_closures_tipadas.dart`.
  *Histórico (até 2026-09-27):* aqui se lia que o lowering "ainda não"
  produzia closures (P1); P1 entrou (`docs/NATIVO-PLANO.md` §7.5).
- **Exceções:** modelo por valor — exceção pendente no runtime e verificação
  depois de cada chamada (NATIVO-PLANO §1), com `try`/`on T`/`catch (e, s)`/
  `finally` e `rethrow`.
- **Estado:** heap, nomes de classe e exceção pendente em `thread_local!` do
  runtime; os globais Dart (`@dfg_<id>` e a bandeira `$ok`) são slots da
  **área de globais do isolado** — cada isolado é uma thread com a sua área,
  como a *field table* da VM (`crates/emit_native/src/llvm/mod.rs:65-72` e
  `1397-1405`; runtime em `crates/runtime/src/gc_raizes.rs`).
  *Histórico (até 2026-09-27):* aqui se lia que os globais eram `@dfg_<id>`
  do módulo e só passariam à tabela do isolado em P8.

---

## 3. Membros do SDK

Os membros do SDK vêm da **fonte do SDK 3.6.2** com a sobreposição
`sdk_nativo/` (`sdk_nativo/libraries.json`) e os natives em Rust da tabela
`crates/emit_native/src/nativos.rs`; o que cada native pendente significa
para a API pública está em `docs/NATIVOS-PENDENTES.md`. Esse é o padrão
desde P5d (`docs/NATIVO-PLANO.md` §7.13).

O runtime escrito à mão que casa membros **pelo nome**
(`crates/emit_native/src/lower/sdk_por_nome.rs`: `length`, `add`,
`substring`, `join`, `Exception(…)`, `StringBuffer()`…) continua **congelado**
e só é usado com `DARTFORGE_SDK_DA_FONTE=0`, para comparação.

> **Histórico (até 2026-09-27).** Esta seção dizia que o runtime por nome
> cobria "só" os membros do SDK e que seria apagado em P5. P5c/P5d entraram
> (`docs/NATIVO-PLANO.md` §7.9 e §7.13); o caminho por nome não foi apagado,
> ficou atrás da variável de ambiente.

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
