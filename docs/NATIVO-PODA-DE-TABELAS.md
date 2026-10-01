# Poda das tabelas de métodos na ligação (AOT de produção)

Especificação da proposta estrutural de tamanho de `PLANO-TAMANHO-DESEMPENHO.md` §1 ("tabelas de
métodos montadas na ligação") e dos itens 1 e 4 de §4 (teste de ausência de código e relatório de
"por que isto ficou"). Escrita em 2026-09-30 sobre `0ae614d8`. `EN/` é
`crates/emit_native/src/`, `RT/` é `crates/runtime/src/`, `VM/` é `E:\references\dart-sdk\`.

## 0. Resumo

Hoje, no `aot --optimize`, toda classe instanciada mantém vivo **todo** membro que ela tem ou
herda, em todas as formas de entrada. A alocação cita a função `df.mt.<lib>.<Classe>`, que devolve
a tabela `df.mt.<lib>.<Classe>$d` com todos os pares (hash do seletor, entrada uniforme). O LTO e o
`/OPT:REF` não têm o que tirar: 86% do `.text` do hello é código Dart do SDK.

Depois desta especificação:

* os módulos do SDK de produção **não definem** as tabelas nem as funções que as devolvem, só as
  declaram;
* cada biblioteca do SDK grava, junto do bitcode em cache, um **resumo**: o que cada função e cada
  global cita, os seletores que cada função chama e o conteúdo das tabelas;
* na ligação do programa, um **ponto fixo por seletor** (RTA) sobre os resumos e o IR do programa
  decide quais entradas vivem: uma entrada da tabela de uma classe viva só entra se o seletor dela
  é chamado por alguma função viva (ou é raiz conservadora, §3.6);
* o módulo do programa passa a definir todas as tabelas, podadas, e o LTO tira o resto.

A poda só existe no AOT de produção. O JIT, a recarga e a DLL de desenvolvimento continuam com as
tabelas inteiras, definidas nos módulos do SDK (§3.8).

## 1. O que existe hoje

| Peça | Onde |
|---|---|
| Tabela de uma classe concreta: pares `(seletor, entrada)` pela linearização | `EN/lower/sdk_fonte.rs` `tabela_de_metodos`, `lower_adaptadores_e_tabelas` |
| Emissão: `@<sim>$d = private … { cid, n, (hash, ptr)… }` e `define ptr @<sim>()` | `EN/llvm/seletores.rs` `emitir_tabelas_de_metodos` |
| Registro preguiçoso na primeira alocação (`dartforge_object_new_t`, `_typed_novo_t`, `_view_nova_t`, `_typed_externo_t`) | `EN/llvm/mod.rs` (alocação em linha e `CallRuntime`), `RT/seletores.rs` |
| Registro na partida: classes do programa, `_StackTrace`, classes dos valores do runtime | `emitir_registro`, `emit_entry` (`df.preparar_isolado`) |
| Busca: cache do ponto de chamada, depois busca binária na tabela | `@df.seletor` em `EN/llvm/mod.rs`, `dartforge_seletor` em `RT/seletores.rs` |
| SDK de produção: bitcode `-O2` por biblioteca, em cache por conteúdo | `EN/sdk_modulo.rs` `sdk_compilado_no_perfil` |
| Ligação de produção: programa + bitcode do SDK + runtime estático, LTO, `/OPT:REF`, `safeicf` | `EN/driver.rs`, `EN/ligador_windows.rs`, `ligador.rs`, `ligador_macos.rs` |

Formas de entrada (`sdk_fonte.rs`, `Adaptador`): `c:m` → `<m>$c` (chamar), `g:x` → `<x>$g` (ler; de
um método, o tear-off), `s:x` → `<x>$s` (gravar), e as tipadas `tc:m`/`ts:x` → `$tc`/`$ts`
(`entrada_tipada.rs`). Cada forma é um par próprio da tabela, com o seu seletor.

## 2. Referências

* **Dart AOT.** `VM/pkg/vm/lib/transformations/type_flow/table_selector_assigner.dart` conta as
  chamadas de cada seletor (`callCount`) e marca o getter de um método como `tornOff`;
  `VM/runtime/vm/compiler/aot/dispatch_table_generator.cc` só põe na tabela global o seletor com
  `IsUsed()`; `VM/runtime/vm/compiler/aot/precompiler.cc` (`DropFunctions`) tira o que não ficou
  retido e guarda o **motivo** de cada retenção (`AddRetainReason`, `RetainReasons`).
* **Swift.** `lib/SILOptimizer/IPO/DeadFunctionElimination.cpp`: um método de vtable vive se algum
  `class_method` alcançável o cita (`ensureAliveClassMethod`); `lib/IRGen/GenMeta.cpp` com
  `VirtualFunctionElimination` deixa o mesmo trabalho para o `GlobalDCE` do LLVM.
* **`crates/mundo`.** O RTA do JS de produção: classes instanciadas × seletores vivos, e raízes
  explícitas para o que o runtime chama por nome.

A diferença do DartForge: o SDK é compilado **uma vez**, antes de qualquer programa. O que o Dart e
o Swift fazem no compilador, aqui acontece na ligação, sobre resumos gravados com o SDK.

## 3. Desenho

### 3.1 Quem poda

Só o perfil de produção com o SDK da fonte (`driver::compile_and_link` com `optimize`, a ligação
`Producao`). O SDK de produção (`PerfilDoSdk::Producao`) é emitido com as tabelas **na ligação**
(`LlvmEmitter::com_tabelas_na_ligacao(true)`); o de desenvolvimento, não.

### 3.2 O SDK de produção sem tabelas

Com `tabelas_na_ligacao`:

* `emitir_tabelas_de_metodos` não escreve nem o `$d` nem a função da tabela;
* quem cita a função da tabela (alocação, `_StackTrace` no registro) a **declara**
  (`declare ptr @df.mt.…()`), como já declara a de outro módulo;
* o conteúdo das tabelas (cid, símbolo, pares `(hash, seletor, entrada)` já ordenados e sem hash
  repetido, `pares_da_tabela`) vai para o resumo (§3.3).

As entradas continuam definidas nos módulos delas, com ligação externa (ou `linkonce_odr`, os
tear-offs): o módulo do programa as cita por nome.

### 3.3 O resumo de uma biblioteca

`<cache nativo>/sdk/<chave>/<biblioteca>.poda`, gravado com o bitcode e parte da mesma entrada do
cache (a chave já inclui o hash do emissor). Texto, uma linha por registro, campos separados por
tabulação:

```
dartforge-poda	1	<biblioteca>
N	<símbolo>                     (tabela de nomes; a posição é o índice)
D	<i>	<L|G>	<refs…>	<hashes…>   (definição: local ou global; índices citados; seletores chamados)
T	<i da função>	<cid>	<hash>:<i da entrada>…
S	<hash>	<texto do seletor>        (para o relatório)
```

O resumo sai do próprio IR emitido (`poda::resumir`), não da HIR: o que o bitcode cita é
exatamente o que o texto cita. A leitura do IR:

* `define … @f(…) {` até a linha `}`: a função `f`; toda `@x` no corpo (e depois do nome, na linha
  do `define`) é referência;
* `@g = …`: global; toda `@x` depois do `=` é referência;
* `call ptr @df.seletor(ptr …, i64 …, i64 <H>, ptr @df.seln.<k>, i64 …)`: a função chama o seletor
  `H`. Se o texto de `@df.seln.<k>` começa com `t` (`tc:m`, `ts:x`), ela também chama o seletor sem
  o `t`: é a volta que `dartforge_seletor` faz quando a classe não tem a entrada tipada;
* símbolo `private`/`internal` é **local**: no resumo recebe o prefixo `<biblioteca>#`, porque
  `@df.seln.0`, `@.str.3` e `@df.classe` se repetem em todo módulo;
* `declare`, metadados, atributos e comdats não entram.

### 3.4 O programa

`poda::montar(ir_do_programa, resumos)` lê o IR do programa com as mesmas regras. As tabelas do
programa (classes do programa, formas de record, `df.mt.$tipo`) continuam emitidas como hoje; a
leitura as reconhece pela linha `@df.mt.…$d = private unnamed_addr constant { … } { i64 <cid>, i64
<n>, i64 <h>, ptr @<entrada>, … }`.

### 3.5 O ponto fixo

Nós: funções, globais e tabelas (a função `df.mt.X` cita o nó-tabela `df.mt.X$d`). Dois conjuntos
que só crescem: **vivos** e **seletores chamados**. Uma fila de trabalho:

1. Tirar um nó da fila; se já vivo, seguir. Marcar vivo.
2. Para cada seletor que ele chama: se novo, marcar chamado e pôr na fila as entradas pendentes
   desse seletor.
3. Se é tabela: cada par `(h, e)` com `h` chamado vai para a fila; senão fica pendente em `h`.
4. Senão, cada referência vai para a fila.

É o `ResolutionWorldBuilder` do dart2js (e o `crates/mundo`) sem restrição por tipo do receptor:
a entrada vive se a classe tem a tabela viva **e** o seletor é chamado em algum lugar. O custo é
linear no grafo.

### 3.6 Raízes conservadoras explícitas

| Mecanismo | Como o runtime chega no código | Raiz |
|---|---|---|
| Entrada do processo | a CRT chama `main`, que passa `dartforge_entry` e `dartforge_dispatch_toString` ao runtime | `main` |
| Chamadas do runtime por símbolo | nenhuma: o runtime estático de produção não cita símbolo Dart (conferido com `llvm-nm --undefined-only` no `dartforge_rtprod_*`); por segurança, todo `dartforge_*` definido num módulo Dart | `dartforge_*` |
| Ajudantes (`_dartforge*` do SDK: erros, `noSuchMethod`, E/S, `Uri.base`) | ponteiros entregues por `dartforge_registrar_ajudante` no `df.registrar.<lib>` | alcançados pelo registro, que a entrada chama |
| `Function.apply`, classe chamável | `dartforge_closure_entry` procura `c:call` pelo hash do texto (`RT/closures.rs`) | seletor `c:call` |
| Seletor tipado sem entrada tipada | `dartforge_seletor` repete a busca com o texto sem o `t` | `tc:m` chamado ⇒ `c:m` chamado (§3.3) |
| `noSuchMethod` | `dartforge_nsm_seletor` → ajudante `_dartforgeNoSuchMethod`, que chama `noSuchMethod` por seletor; os encaminhadores gerados são pares comuns da tabela | pelo ajudante; `c:noSuchMethod` também é raiz |
| Protocolo de `Object` que o runtime usa indiretamente (`toString` do erro não tratado, igualdade e hash das coleções) | `dartforge_dispatch_toString`, código do SDK | `c:toString`, `c:==`, `g:hashCode`, `c:noSuchMethod`, `g:runtimeType` (baratos: quase toda classe viva já os tem vivos) |
| Tear-off de método | `obj.m` sem chamada é o seletor `g:m`, que chama a entrada `$g` (que cria o `$tearm`) | pelo seletor, sem raiz extra |
| Tear-off de função e de construtor, closures | referência direta no IR | sem raiz extra |
| FFI | callbacks e trampolins registrados em `df.preparar_isolado` (`dartforge_ffi_registrar_*`); `@Native` é importação | pela entrada |
| Isolados | `Isolate.spawn` leva uma closure; a thread nova chama `df.preparar_isolado`, passado por `dartforge_registrar_isolados`; as tabelas copiadas entre isolados (`RT/portas.rs`) são do mesmo executável | pela entrada |
| Mirrors | o `dart:mirrors` nativo só lê nome e biblioteca da classe (`sdk_nativo/mirrors/mirrors_patch.dart`); não há invocação por nome | nenhuma. Se um dia houver `invoke`/`getField`, a função do runtime que busca por nome entra em `CHAMA_POR_NOME` (§3.7) |
| `llvm.used`, `llvm.global_ctors` | o LLVM | todo global `llvm.*` |

Regra geral: uma busca por hash calculado em tempo de execução, fora de `df.seletor`, **precisa**
de uma raiz aqui. Hoje há só a de `c:call`.

### 3.7 Busca por nome em tempo de execução

`poda::CHAMA_POR_NOME` lista as funções do runtime que buscam na tabela por um nome que só existe
em tempo de execução. Se alguma é citada por função viva, **toda** entrada de tabela viva fica
(o modelo de antes). A lista começa vazia; ela existe para que o próximo mecanismo dinâmico não
quebre a poda em silêncio.

### 3.8 A saída e o que fica de fora

O `montar` devolve o IR do programa com:

* cada tabela do programa reescrita com os pares vivos (tabela morta: `{ cid, 0 }`);
* para cada tabela dos resumos, a função `define ptr @df.mt.X()` e o `@df.mt.X$d` privado,
  podado; a declaração `declare ptr @df.mt.X()` que o programa tinha sai;
* `declare i64 @<entrada>(i64, ptr, ptr)` de cada entrada viva que o programa não declara nem
  define.

Tabela morta (a função dela não é alcançada) sai vazia: nada vivo a cita, e o LTO a tira. Tabela
viva registra só os pares vivos; como toda busca da tabela usa um hash de `df.seletor` vivo (ou
`c:call`), um par podado nunca é procurado.

Ficam de fora, sem mudança:

* **JIT e recarga:** a geração nova do programa pode chamar qualquer seletor; o JIT usa a DLL de
  desenvolvimento, com as tabelas inteiras nos módulos do SDK, e `dartforge_publicar_geracao`
  substitui tabelas do programa. Podar ali exigiria reconstruir o SDK a cada recarga.
* **`--nativo` sem `--optimize` e a DLL do SDK:** a ligação rápida contra a DLL; a DLL é uma só
  para todo programa.

### 3.9 Formas de entrada sob demanda

Como cada forma é um par próprio com o seu seletor, o ponto fixo já as poda uma a uma: `$tearm`
(e o `$g` de um método) só com `g:m` chamado; `$c` só com `c:m`; `$tc`/`$ts` só com o seletor
tipado; `$s` só com `s:x`. O corpo do método continua vivo se alguma chamada direta o cita. Os
adaptadores continuam compilados no bitcode do SDK (o custo é da compilação a frio, uma vez); o
LTO os tira do executável.

### 3.10 Relatório "por que isto ficou"

`DARTFORGE_POR_QUE=<arquivo>` na compilação de produção grava, para cada nó vivo, a causa da
primeira vez em que ele entrou:

```
<símbolo>	raiz	-	-
<símbolo>	ref	<quem citou>	-
<entrada>	tabela	<tabela>	<seletor>	<primeira função viva que chamou o seletor>
```

`tools/tamanho/por-que.py` segue a cadeia até a raiz ("quem puxou este método?") e soma, com o
mapa da ligação, os bytes por causa. `DARTFORGE_MAPA_DA_LIGACAO=1` pede o mapa ao ligador
(`<saída>.map`); `tools/tamanho/quebra.py` quebra o `.text` por biblioteca.

`--timings` mostra a poda: tabelas vivas, pares vivos de quantos, tempo.

### 3.11 Chave de comparação

`DARTFORGE_SEM_PODA_DE_TABELAS=1` monta as tabelas com todos os pares (o modelo de antes, mas com a
mesma estrutura de módulos): serve para medir o ganho e para isolar um erro. Só o `montar` a lê; a
chave do SDK não muda.

### 3.12 O programa sem o que não foi alcançado

Com a poda, o `montar` também tira do texto do programa as definições (funções e globais) que o
ponto fixo não alcançou. É a mesma decisão que a LTO e o `/OPT:REF` tomariam, com as mesmas
raízes (`main`, `dartforge_*`, `llvm.*`) e as mesmas arestas (todo `@nome` citado). A diferença
está em quando ela acontece: antes da otimização e da geração de código, e não depois delas.

No `new_sali/backend` saem 131 270 funções e 77 633 globais: 476 MB dos 1,31 GB de IR
(`docs/NATIVO-PRODUCAO-GRANDE.md` §1.3). `--timings` mostra a contagem na linha `Poda:`. Teste:
`poda::testes::definicoes_mortas_do_programa_saem_do_ir`. `DARTFORGE_SEM_PODA_DE_TABELAS=1`
desliga também isto.

O despacho compacto do programa grande (`docs/NATIVO-PRODUCAO-GRANDE.md` §3) chama
`@df.seletor_d(ptr <cache>, i64 <receptor>, ptr @df.seld.<k>)`, e o descritor
`@df.seld.<k> = … { i64 <hash>, ptr @df.seln.<k>, i64 <len> }` leva o seletor: o `resumir` lê o
hash do descritor citado (`descritor_de_seletor`, `chamada_de_seletor_compacta`; teste
`chamada_compacta_le_o_seletor_do_descritor`).

## 4. Verificação

* **Teste de ausência** (o `deadstrip.test.ts` do scriptc): `corpus/nativo/81_poda_de_tabelas.dart`
  tem uma classe **instanciada** com métodos, getter, setter e tear-off nunca usados (nomes
  únicos) e outros usados de cada forma. O teste `poda_tira_membros_nao_usados` (ignorado por
  padrão, como `producao_e_um_executavel_autocontido`) compila em produção com o mapa da ligação e
  confere que nem o corpo nem `$c`/`$tc`/`$g`/`$s`/`$tearm` dos não usados estão no mapa, que os
  usados estão, e que a saída é a esperada; o harness compara com a VM.
* Testes de unidade do `poda.rs`: leitura do IR (locais, seletores, tipados, tabelas), resumo ida e
  volta, ponto fixo e montagem em módulos pequenos.
* O corpus nativo inteiro em `aot --optimize` contra a VM; `--nativo`, `--jit`,
  `--nativo --gc-stress --limite-exec 60`; o corpus JS pelo AOT; `cargo test --workspace`.
* Tamanho (hello, t0, servidor, json, blend), tempo de ligação e desempenho (json, blend) antes e
  depois.

## 5. Arquivos

| Arquivo | Mudança |
|---|---|
| `EN/poda.rs` (novo) | leitura do IR, resumo, ponto fixo, montagem, relatório, testes |
| `EN/llvm/seletores.rs` | `pares_da_tabela`; tabelas na ligação |
| `EN/llvm/mod.rs` | `com_tabelas_na_ligacao` |
| `EN/sdk_modulo.rs` | tabelas no `BibliotecaDoSdk`, `.poda` no perfil de produção, `SdkCompilado::resumos` |
| `EN/driver.rs` | `montar` na ligação de produção; `--timings` |
| `EN/ligador.rs`, `ligador_windows.rs`, `ligador_macos.rs` | `DARTFORGE_MAPA_DA_LIGACAO` |
| `corpus/nativo/81_poda_de_tabelas.dart` | o caso do teste de ausência |
| `tools/tamanho/` | `quebra.py`, `por-que.py` |

## 6. Limites

* Sem restrição pelo tipo do receptor: `toString` chamado em qualquer lugar mantém o `toString` de
  toda classe viva. A restrição do `crates/mundo` pede os tipos no resumo; fica para depois.
* Os ajudantes `_dartforge*` são raízes por biblioteca: `_dartforgeIniciarIo` mantém o preparo do
  `dart:io` em todo programa. Tirá-los pede saber quais o programa pode disparar.
* As funções que só uma tabela podada citava continuam no bitcode do SDK; quem as tira é o LTO.
