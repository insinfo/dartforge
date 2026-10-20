# Poda das tabelas de métodos na ligação (AOT de produção)

Especificação da proposta estrutural de tamanho de `PLANO-TAMANHO-DESEMPENHO.md` §1 ("tabelas de
métodos montadas na ligação") e dos itens 1 e 4 de §4 (teste de ausência de código e relatório de
"por que isto ficou"). Escrita em 2026-09-30 sobre `0ae614d8`. `EN/` é
`crates/emit_native/src/`, `RT/` é `crates/runtime/src/`, `VM/` é `E:\references\dart-sdk\`.

## Atualização de 20/10/2026: relocações de base PE

A inspeção de imagens agora lê o diretório de relocações de base PE32+ em
Windows x64/ARM64, sem depender do nome da seção. Confere mapeamentos, blocos e
spans de escrita HIGHLOW/DIR64; entradas ABSOLUTE são preenchimento. Escritas
sobre cabeçalhos ou a própria tabela, tipos desconhecidos e mapeamentos ambíguos
são recusados. Os outros diretórios do loader permanecem observações abertas.

Imagens recém-ligadas das duas arquiteturas tiveram 2.100 escritas DIR64 em
cinco blocos, sem atingir strings. No explorador Windows preservado, as 51.190
escritas declaradas não sobrepõem os 871.040 bytes de strings estáticas. Nenhum
arquivo foi executado nesta inspeção. Imports/TLS/configuração do loader,
inicializadores nativos, marcadores e limites realmente registrados ainda não
estão provados; a inicialização do SDK e a poda de backends continuam pendentes.
[Evidências](../bench/results/2026-10-20-pe-base-relocations/README.md).

## Atualização de 20/10/2026: ordem de visita da dominância

A validação estrutural reutiliza uma travessia iterativa em ordem reversa de
pós-visita do fluxo, em vez de ordenar as interseções pela posição física dos
blocos. A equação do ponto fixo, os limites de 512 blocos/8.192 instruções e as
recusas conservadoras permanecem iguais. A travessia evita recursão e conserva
apenas uma aresta ativa por quadro, mesmo com destinos repetidos de switch.

No teste controlado de 512 blocos em ordem física inversa, as varreduras caíram
de 511 para duas, preservando todos os conjuntos. O oráculo independente por
remoção de blocos e novos grafos cíclicos com rótulos embaralhados conferem a
correção. O IR completo do explorador ficou idêntico entre os compiladores;
isso não demonstra ganho geral de compilação nem completa a poda de backends.
[Evidências](../bench/results/2026-10-20-dominator-traversal/README.md).

## Atualização de 19/10/2026: custo da validação estrutural

A prova estrutural de SSA usada pela proveniência física calcula dominância
com bitmaps de até 512 blocos. O ponto fixo, os predecessores alcançáveis e
as recusas de HIR inválido permanecem iguais; corpos acima desse orçamento
continuam sem prova. A representação evita alocar árvores em cada interseção.

A validação inclui todos os grafos dirigidos de até quatro blocos sem aresta
de entrada, comparados com um oráculo independente que remove blocos e verifica
caminhos. Há casos com 512 blocos, ciclos, predecessores inalcançáveis, arestas
duplicadas e o limite do orçamento. A medição isolada de 256 blocos compara as
duas representações e confere todos os pares de dominância; não demonstra ganho
de tempo da compilação inteira. Evidências e observação de diferença entre cache
antigo e SDK reconstruído:
[registros](../bench/results/2026-10-19-dense-dominators/README.md).

A diferença de cache foi reproduzida com `DARTFORGE_OPTIMIZE_HIR=0`: o modo não
entrava na chave do SDK compilado. A correção separa os dois perfis por política
HIR, conforme os [registros da investigação](../bench/results/2026-10-19-hir-sdk-cache-policy/README.md).

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
| Registro na partida: classes do programa, `_StackTrace`, classes dos valores do runtime | `emitir_registro`, `emit_entry` (`df.prepare_isolate`) |
| Busca: cache do ponto de chamada, depois busca binária na tabela | `@df.selector` em `EN/llvm/mod.rs`, `dartforge_selector` em `RT/seletores.rs` |
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
* **`crates/world`.** O RTA do JS de produção: classes instanciadas × seletores vivos, e raízes
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
dartforge-poda	2	<biblioteca>
N	<símbolo>                     (tabela de nomes; a posição é o índice)
D	<i>	<L|G>	<refs…>	<hashes…>   (definição: local ou global; índices citados; seletores chamados)
T	<i da função>	<cid>	<hash>:<i da entrada>…
S	<hash>	<texto do seletor>        (para o relatório)
R	<i>	<receita RTI com F<…>>       (as assinaturas da FFI, §3.14)
```

O resumo sai do próprio IR emitido (`poda::resumir`), não da HIR: o que o bitcode cita é
exatamente o que o texto cita. A leitura do IR:

* `define … @f(…) {` até a linha `}`: a função `f`; toda `@x` no corpo (e depois do nome, na linha
  do `define`) é referência;
* `@g = …`: global; toda `@x` depois do `=` é referência;
* `call ptr @df.selector(ptr …, i64 …, i64 <H>, ptr @df.seln.<k>, i64 …)`: a função chama o seletor
  `H`. Se o texto de `@df.seln.<k>` começa com `t` (`tc:m`, `ts:x`), ela também chama o seletor sem
  o `t`: é a volta que `dartforge_selector` faz quando a classe não tem a entrada tipada;
* símbolo `private`/`internal` é **local**: no resumo recebe o prefixo `<biblioteca>#`, porque
  `@df.seln.0`, `@.str.3` e `@df.class` se repetem em todo módulo;
* `declare`, metadados, atributos e comdats não entram.

### 3.4 O programa

`poda::montar(ir_do_programa, resumos)` lê o IR do programa com as mesmas regras. As tabelas do
programa (classes do programa, formas de record, `df.mt.$type`) continuam emitidas como hoje; a
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

É o `ResolutionWorldBuilder` do dart2js (e o `crates/world`) sem restrição por tipo do receptor:
a entrada vive se a classe tem a tabela viva **e** o seletor é chamado em algum lugar. O custo é
linear no grafo.

### 3.6 Raízes conservadoras explícitas

| Mecanismo | Como o runtime chega no código | Raiz |
|---|---|---|
| Entrada do processo | a CRT chama `main`, que passa `dartforge_entry` e `dartforge_dispatch_toString` ao runtime | `main` |
| Chamadas do runtime por símbolo | nenhuma: o runtime estático de produção não cita símbolo Dart (conferido com `llvm-nm --undefined-only` no `dartforge_rtprod_*`); por segurança, todo `dartforge_*` definido num módulo Dart | `dartforge_*` |
| Ajudantes (`_dartforge*` do SDK: erros, `noSuchMethod`, E/S, `Uri.base`) | ponteiros entregues por `dartforge_register_helper` no `df.register.<lib>` | alcançados pelo registro, que a entrada chama |
| `Function.apply`, classe chamável | `dartforge_closure_entry` procura `c:call` pelo hash do texto (`RT/closures.rs`) | seletor `c:call` |
| Seletor tipado sem entrada tipada | `dartforge_selector` repete a busca com o texto sem o `t` | `tc:m` chamado ⇒ `c:m` chamado (§3.3) |
| `noSuchMethod` | `dartforge_nsm_selector` → ajudante `_dartforgeNoSuchMethod`, que chama `noSuchMethod` por seletor; os encaminhadores gerados são pares comuns da tabela | pelo ajudante; `c:noSuchMethod` também é raiz |
| Protocolo de `Object` que o runtime usa indiretamente (`toString` do erro não tratado, igualdade e hash das coleções) | `dartforge_dispatch_toString`, código do SDK | `c:toString`, `c:==`, `g:hashCode`, `c:noSuchMethod`, `g:runtimeType` (baratos: quase toda classe viva já os tem vivos) |
| Tear-off de método | `obj.m` sem chamada é o seletor `g:m`, que chama a entrada `$g` (que cria o `$tearm`) | pelo seletor, sem raiz extra |
| Tear-off de função e de construtor, closures | referência direta no IR | sem raiz extra |
| FFI | callbacks e trampolins registrados em `df.prepare_isolate` (`dartforge_ffi_register_*`); `@Native` é importação | pela entrada |
| Isolados | `Isolate.spawn` leva uma closure; a thread nova chama `df.prepare_isolate`, passado por `dartforge_register_isolates`; as tabelas copiadas entre isolados (`RT/portas.rs`) são do mesmo executável | pela entrada |
| Mirrors | o `dart:mirrors` nativo só lê nome e biblioteca da classe (`native_sdk/mirrors/mirrors_patch.dart`); não há invocação por nome | nenhuma. Se um dia houver `invoke`/`getField`, a função do runtime que busca por nome entra em `CHAMA_POR_NOME` (§3.7) |
| `llvm.used`, `llvm.global_ctors` | o LLVM | todo global `llvm.*` |

Regra geral: uma busca por hash calculado em tempo de execução, fora de `df.selector`, **precisa**
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
viva registra só os pares vivos; como toda busca da tabela usa um hash de `df.selector` vivo (ou
`c:call`), um par podado nunca é procurado.

Ficam de fora, sem mudança:

* **JIT e recarga:** a geração nova do programa pode chamar qualquer seletor; o JIT usa a DLL de
  desenvolvimento, com as tabelas inteiras nos módulos do SDK, e `dartforge_publish_generation`
  substitui tabelas do programa. Podar ali exigiria reconstruir o SDK a cada recarga.
* **`--native` sem `--optimize` e a DLL do SDK:** a ligação rápida contra a DLL; a DLL é uma só
  para todo programa.

### 3.9 Formas de entrada sob demanda

Como cada forma é um par próprio com o seu seletor, o ponto fixo já as poda uma a uma: `$tearm`
(e o `$g` de um método) só com `g:m` chamado; `$c` só com `c:m`; `$tc`/`$ts` só com o seletor
tipado; `$s` só com `s:x`. O corpo do método continua vivo se alguma chamada direta o cita. Os
adaptadores continuam compilados no bitcode do SDK (o custo é da compilação a frio, uma vez); o
LTO os tira do executável.

### 3.10 Relatório "por que isto ficou"

`DARTFORGE_WHY=<arquivo>` na compilação de produção grava, para cada nó vivo, a causa da
primeira vez em que ele entrou:

```
<símbolo>	raiz	-	-
<símbolo>	ref	<quem citou>	-
<entrada>	tabela	<tabela>	<seletor>	<primeira função viva que chamou o seletor>
```

`tools/size/why.py` segue a cadeia até a raiz ("quem puxou este método?") e soma, com o
mapa da ligação, os bytes por causa. `DARTFORGE_LINK_MAP=1` pede o mapa ao ligador
(`<saída>.map`); `tools/size/breakdown.py` quebra o `.text` por biblioteca.

`--timings` mostra a poda: tabelas vivas, pares vivos de quantos, tempo.

### 3.11 Chave de comparação

`DARTFORGE_NO_TABLE_PRUNING=1` monta as tabelas com todos os pares (o modelo de antes, mas com a
mesma estrutura de módulos): serve para medir o ganho e para isolar um erro. Só o `montar` a lê; a
chave do SDK não muda.

### 3.12 O programa sem o que não foi alcançado

Com a poda, o `montar` também tira do texto do programa as definições (funções e globais) que o
ponto fixo não alcançou. É a mesma decisão que a LTO e o `/OPT:REF` tomariam, com as mesmas
raízes (`main`, `dartforge_*`, `llvm.*`) e as mesmas arestas (todo `@nome` citado). A diferença
está em quando ela acontece: antes da otimização e da geração de código, e não depois delas.

No `new_sali/backend` saem 131 270 funções e 77 633 globais: 476 MB dos 1,31 GB de IR
(`docs/NATIVO-PRODUCAO-GRANDE.md` §1.3). `--timings` mostra a contagem na linha `Poda:`. Teste:
`poda::testes::definicoes_mortas_do_programa_saem_do_ir`. `DARTFORGE_NO_TABLE_PRUNING=1`
desliga também isto.

O despacho compacto do programa grande (`docs/NATIVO-PRODUCAO-GRANDE.md` §3) chama
`@df.selector_d(ptr <cache>, i64 <receptor>, ptr @df.seld.<k>)`, e o descritor
`@df.seld.<k> = … { i64 <hash>, ptr @df.seln.<k>, i64 <len> }` leva o seletor: o `resumir` lê o
hash do descritor citado (`descritor_de_seletor`, `chamada_de_seletor_compacta`; teste
`chamada_compacta_le_o_seletor_do_descritor`).

### 3.13 A poda da HIR, antes da otimização e da emissão

`poda::podar_hir` faz o ponto fixo de §3.5 antes, sobre o módulo HIR do programa recém-baixado.
O resumo do programa é tirado da própria HIR (`resumo_da_hir`), com os resumos das bibliotecas
do SDK do perfil. Saem:

* as funções que o grafo não alcança;
* os pares de tabela de seletor que ninguém chama.

Essas funções deixam de passar pela otimização da HIR, pela emissão e pela geração de código.

* **Arestas do resumo da HIR:**
  * `CallStatic`;
  * o código das closures e dos tear-offs;
  * `Constant::Funcao`;
  * o nome do `CallRuntime`;
  * a tabela citada por uma alocação;
  * os seletores dos `CallSeletor`, com a volta do tipado.
* **Raízes:** a entrada sintética `dartforge_entry`, que cita o que o emissor acrescenta sem
  instrução — o `main`, os registros, as tabelas do programa, o `toString` e as vtables, a FFI,
  os ajudantes e as raízes da fonte.

O desenvolvimento também grava os resumos do SDK (`<lib>.poda`, ao lado dos objetos da DLL),
então a poda vale nos dois perfis. Ela não vale no JIT, porque a geração seguinte de uma
recarga pode chamar qualquer seletor.

Testes: `poda_da_hir_tira_o_que_nao_alcanca` e `poda_da_hir_ve_o_seletor_chamado_pelo_sdk`.
`DARTFORGE_NO_HIR_PRUNING=1` desliga.

### 3.14 Os trampolins da FFI pelas assinaturas pedidas

`lower_ffi` gera um trampolim (`df.ffi.<chave>$ent`) e um callback (`$cb`) para **toda**
assinatura nativa da tabela de tipos, inclusive a do código morto. No new_sali/backend
(com o `openssl_bindings` gerado) são 3 591 assinaturas e 7 182 funções, 17,6 MB de IR (3% do
programa), todas raízes da poda: o registro do isolado cita todas.

O Dart exige o tipo nativo constante no sítio de `asFunction`, `lookupFunction`,
`Pointer.fromFunction` e `NativeCallable` (o transformador de FFI do front-end recusa um
genérico). Então a assinatura que o runtime vai procurar (`ffi.rs::chave_da_assinatura`, pela
RTI) está escrita numa receita RTI do código que chama: o termo `F<0;R;n;P1,…;>` dentro da
receita do argumento de tipo (sozinho, na tupla `L<…>` ou em `NativeFunction<…>`).

* **Receitas no resumo.** O resumo leva as receitas com `F<` e o nó que as tem (linha `R`):
  * na HIR do programa, o `Const(String)` da função (os getters `df.rti.*`);
  * no IR das bibliotecas do SDK (`resumir`), a constante do texto (`df.s.*`, ou `.str.N` com
    `F\3C`).
* **Ponto fixo.** `podar_hir` põe como raízes só os trampolins e callbacks das chaves achadas
  nas receitas de nós vivos (`chaves_ffi_da_receita`: retorno e parâmetros que são classes da
  tabela de tipos nativos, `S<rti>.` das structs por valor, `*` do `VarArgs`; nada de opcional,
  nomeado ou genérico). Um trampolim vivo alcança código novo, então o ponto fixo repete até as
  chaves pararem de crescer. As listas `ffi_trampolins` e `ffi_callbacks` do módulo ficam só
  com as chaves vivas, e o registro do isolado só as registra.
* **Reuso do resumo (20/10/2026).** `prune_hir` constrói os fatos dos corpos e
  tabelas uma vez por módulo imutável; cada rodada substitui apenas a entrada e
  os nomes exclusivos das raízes FFI. O empréstimo do módulo termina antes das
  mutações; uma nova poda após especialização refaz toda a análise. O resumo
  completo continua igual ao da reconstrução por rodada. O caso transitivo com
  quatro rodadas passou de quatro construções dos corpos para uma; o explorador
  real usa duas rodadas. `--timings` informa a contagem, incluindo a rodada final
  estável. Não há nova permissão de poda nem cache global. Evidências:
  [reuso do resumo HIR](../bench/results/2026-10-20-hir-summary-reuse/README.md).
* **Medida (new_sali/backend):** as receitas do IR inteiro dão 44 chaves, todas registradas
  (nenhuma chave achada fica sem trampolim).

Testes: `testes_ffi::chave_da_assinatura_na_receita` e `receitas_do_ir_vao_ao_resumo`; o corpus
de FFI (`08`, `12`, `13`, `15`–`19`, `128`) nos quatro modos. `DARTFORGE_NO_FFI_PRUNING=1`
desliga.

## 4. Verificação

* **Teste de ausência** (o `deadstrip.test.ts` do scriptc): `corpus/native/81_poda_de_tabelas.dart`
  tem uma classe **instanciada** com métodos, getter, setter e tear-off nunca usados (nomes
  únicos) e outros usados de cada forma. O teste `poda_tira_membros_nao_usados` (ignorado por
  padrão, como `producao_e_um_executavel_autocontido`) compila em produção com o mapa da ligação e
  confere que nem o corpo nem `$c`/`$tc`/`$g`/`$s`/`$tearm` dos não usados estão no mapa, que os
  usados estão, e que a saída é a esperada; o harness compara com a VM.
* Testes de unidade do `poda.rs`: leitura do IR (locais, seletores, tipados, tabelas), resumo ida e
  volta, ponto fixo e montagem em módulos pequenos.
* O corpus nativo inteiro em `aot --optimize` contra a VM; `--native`, `--jit`,
  `--native --gc-stress --exec-limit 60`; o corpus JS pelo AOT; `cargo test --workspace`.
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
| `EN/ligador.rs`, `ligador_windows.rs`, `ligador_macos.rs` | `DARTFORGE_LINK_MAP` |
| `corpus/native/81_poda_de_tabelas.dart` | o caso do teste de ausência |
| `tools/size/` | `breakdown.py`, `why.py` |

## 6. Limites

* Sem restrição pelo tipo do receptor: `toString` chamado em qualquer lugar mantém o `toString` de
  toda classe viva. A restrição do `crates/world` pede os tipos no resumo; fica para depois.
* Os ajudantes `_dartforge*` são raízes por biblioteca: `_dartforgeStartIo` mantém o preparo do
  `dart:io` em todo programa. Tirá-los pede saber quais o programa pode disparar.
* As funções que só uma tabela podada citava continuam no bitcode do SDK; quem as tira é o LTO.

## 7. Alcance fechado experimental — atualização de 19/10/2026

`DARTFORGE_CLOSED_CLASS_REACHABILITY=1` tenta provar quais tabelas do programa
podem ficar ativas. A análise exige uma imagem fixa sem ARC preparado ou fronteiras nativas
desconhecidas. O leitor de `allocation-inventory.json` agora admite importações
restritas: fechamentos vazios, sem exposição, com corpos completos e chamadas
diretas de ABI compatível e resultados escalares/void. Retornos gerenciados ou
ponteiros exigem certificados adicionais de valor/região. Alocação conhecida,
sozinha, não autoriza poda.
Chamadas nativas, registro de imagens, memória bruta e funções interponíveis
continuam desconhecidos. Os casos fora dessa cobertura mantêm o registro amplo.

Com a opção experimental ativa, `DARTFORGE_TRACE_CLASS_REACHABILITY=1`
mostra em stderr a primeira fronteira que a análise não conseguiu provar:
configuração, consistência das declarações, inventário, raiz importada,
símbolo alcançável, corpo/ABI, instrução ou endereço de função exposto.
O diagnóstico identifica funções, símbolos e instruções quando disponíveis.
Ele não muda a decisão de poda: qualquer rejeição descarta o conjunto temporário
de classes e mantém o registro amplo. Sem a opção de trace, a rejeição é silenciosa.

Ao iniciar uma nova poda HIR, o certificado anterior é descartado **antes** de
construir o grafo de fallback e calcular as assinaturas FFI vivas. Assim, uma
prova recusada ou a opção experimental desligada usa as raízes amplas do HIR
atual, sem herdar raízes reduzidas da análise anterior. Isso não reconstrói
corpos já removidos: mudanças semânticas após uma poda destrutiva exigem o HIR
completo novamente.
A primeira fronteira pode ocultar outras; resolver uma mensagem não prova que
as etapas seguintes estejam cobertas.

Declarações de funções de tabela importadas agora são distintas das tabelas
locais. O inventário deriva pares de símbolo/CID dos resumos conferidos:
exige declaração externa única, definição virtual que referencia exatamente
seus dados, nome dos dados e CID sem colisões. A análise só usa essa identidade
após reconciliar o inventário com os resumos efetivamente selecionados e os
símbolos/classes locais. Isso permite uma declaração importada não alcançada;
não autoriza chamá-la nem criar objetos dessa classe. Corpos alcançáveis,
alocações e inicialização do SDK continuam exigindo provas independentes.
Na validação com Dart 3.6.2, 662 declarações foram identificadas; a inicialização
continua aberta. O teste real de poda ainda rejeita os metadados de `Object`,
CID 0, sem tabela própria. Esse resultado histórico motivou a distinção explícita
entre metadados nominais e classes alocáveis descrita a seguir; o teste de remoção
dos backends não foi alterado.

O lowering agora marca ancestrais puramente nominais: classes abstratas, mixins e
a raiz sintética inserida diretamente em `lower_program`. A prova confere essa
premissa: cada CID precisa existir, ter ancestralidade conhecida e não possuir
tabela, função de tabela nem identidade criada implicitamente pelo runtime.
Uma classe concreta que perdeu a tabela continua inválida; a ausência da tabela
não produz a premissa. Todas as alocações alcançadas ainda exigem layout e tabela
locais. Os metadados e as arestas de subtipo permanecem no módulo e na emissão.

A consulta `dartforge_is_subclass` também pode participar da prova, com ABI exata
`i8(i64, i64)` e operandos HIR `I64`; referências não são aceitas como CIDs só
porque usam o mesmo tipo LLVM. Os atributos da declaração do catálogo não mudam
a comparação do protótipo. A implementação do runtime consulta o grafo e pode
preparar caches em memória nativa; não cria objetos Dart nem altera o grafo de
classes. O teste compilado compara registro amplo e fechado, com consulta
transitiva positiva e alvo nominal não relacionado, em execução normal e sob
estresse de GC. A marcação de metadados não autoriza registros desconhecidos,
entradas externas, corpos de protocolo ou fronteiras importadas ainda não provados.

O leitor verifica a mesma publicação contra o compilador, o runtime e os objetos
selecionados; relê os bytes de `.poda` com hash conferido, reconcilia os resumos
usados no grafo e recalcula os fechamentos por componentes fortemente conexos.
Recusa resumos obsoletos, colisões com símbolos/classes do programa, ABI divergente
e metadados inconsistentes. Declarações globais repetidas ficam opacas. A confiança
é no cache imutável pertencente ao compilador; hashes não autenticam um cache
inteiramente falsificado. A leitura adicional só ocorre com a opção experimental.
No SDK real de produção foram reconhecidas 428 funções com fechamento vazio
verificável, mas a inicialização completa continua sem cobertura. Testes sintéticos
de grafo usam artefatos explicitamente não executáveis; não certificam execução
de uma aplicação importada. Evidências:
`bench/results/2026-10-19-sdk-inventory-consumption/README.md`.

A prova agora exige destinos alcançáveis com definição conhecida, raízes com
corpos disponíveis, assinaturas compatíveis nas chamadas locais e nos auxiliares
admitidos do runtime, entradas uniformes `Ref(Ref, Ptr, Ptr)` e corpos com SSA/CFG
verificáveis. Reutiliza a verificação estrutural limitada de
`pruning/receiver_provenance.rs`; não substitui um verificador geral de tipos da
HIR. Escritas/leitura de memória nativa, endereços de funções e bindings externos
continuam desconhecidos. A memória local aceita é a de slots `Alloca` identificados
na própria função e globais declaradas no módulo, com representação compatível;
loads de `Ref` por ponteiro continuam fora da prova. A reanálise que falha descarta
o conjunto anterior e o conjunto temporário de tabelas.

Closures uniformes locais e tear-offs agora têm uma prova limitada de identidade
do endereço de código. O destino precisa ter corpo no módulo e assinatura
`Ref(Ref, Ptr, Ptr)`; seu handle só pode servir à invocação direta da mesma
definição SSA ou à leitura do contexto capturado. Escapes por argumentos,
globais, retornos, aliases ou escritas mantêm o registro amplo. O receptor
implícito da entrada também fica protegido: a assinatura uniforme, sozinha, não
autoriza invocá-lo, pois o mesmo corpo pode ser alcançado por seletor ou chamada
estática. Closures encaminhadas e vindas do SDK continuam fora dessa prova.
As alocações transitivas do corpo local ativam suas tabelas no mesmo grafo. O
suporte não elimina as fronteiras ainda abertas da inicialização do SDK.

Closures tipadas locais agora também podem fechar essa fronteira. As duas
entradas precisam ter corpo no módulo: a uniforme continua exigindo
`Ref(Ref, Ptr, Ptr)` e a tipada exige contexto `Ref`, até 12 argumentos nas
representações `Ref`/`I64`/`F64`/`I1` e resultado em uma dessas representações.
O código ABI armazenado na criação precisa coincidir com a assinatura do corpo.
O endereço tipado só pode vir de uma leitura direta do campo de código do mesmo
handle SSA, por `GetField` ou `dartforge_object_get`, e servir à chamada tipada;
não pode ser armazenado, retornado, transformado ou passado como argumento.
Cada chamada confere quantidade e representação dos argumentos e resultado.
Leituras de contexto, código ABI e classe do handle são permitidas; escritas e
escapes continuam recusados. O layout vem das constantes do runtime.

A fixture tipada compara registro amplo/reduzido em 32 binários e 64 execuções
com GC normal/forçado, incluindo argumentos e resultados `int`/`double`/`bool` e contexto capturado.
O caso com resultado objeto verifica despacho por seletor sobre a classe criada
pelo corpo tipado, preservando sua tabela e removendo a classe sem uso.
Dezesseis variantes negativas cobrem ABI, assinatura, endereço forjado, mutação,
leitura de ponteiro como objeto e escape. É uma fixture de ABI sem SDK, não
certificação do aplicativo completo nem remoção dos backends estrangeiros.
Evidências: `bench/results/2026-10-19-closed-typed-closures/README.md`.

Tabelas das classes canônicas do runtime são protegidas independentemente do vetor
opcional de registro do módulo: boxing, strings e exceções podem criar esses
valores sem uma instrução `AllocObject`. Com cobertura completa, a emissão omite
tanto os registros quanto as definições de tabela inativas. O catálogo nominal de
classes permanece disponível.

Os testes incluem executáveis do mesmo módulo com registro amplo e com o conjunto
fechado, exercitando despacho por seletor nos dois modos com e sem `GC_STRESS`.
Essa fixture não importa SDK e usa o `main` do runtime no lugar do wrapper do
emissor, que normalmente faz parte da ligação com o SDK. Ela não certifica a
poda de um aplicativo completo com SDK.

Uma segunda fixture executa closure uniforme, tear-off e contexto capturado com
registro amplo/reduzido e GC normal/forçado: seis binários e 12 execuções. O
despacho por seletor sobre o resultado verifica que a tabela necessária permanece;
as tabelas sem uso saem do IR. Dezesseis variantes de ABI, escape, mutação e
receptor desconhecido recusam o fechamento. Evidências:
`bench/results/2026-10-19-closed-uniform-closures/README.md`.

O teste `production_profile::platform_constant_prunes_the_other_backends` continua
como exigência de remoção completa dos outros backends. O suporte às importações
e suas exposições ainda precisa ser completado; a configuração padrão mantém as
tabelas desse caso. Evidências da rodada:
`bench/results/2026-10-19-class-coverage/README.md`.


### Metadados de frames locais

A declaração de `_AsyncFrame` acompanha agora as alocações locais que restaram
após a poda de fontes. A flag do loop de eventos continua controlando seu callback
e a chamada do loop; importar facilidades assíncronas sem criar frames locais
não registra uma classe fictícia no programa. Geradores baixados diretamente
sem a flag também recebem os metadados dos frames que realmente criam.
O ABI variável dos frames e os registros por corpo de retomada permanecem.
Isso não prova a ancestralidade nem fecha a exposição dos frames alocados.
No caso original de poda de plataforma, a primeira rejeição passa a `_Type`,
com ancestralidade ainda não provada. A remoção dos backends continua pendente.
Evidências: `bench/results/2026-10-19-local-frame-metadata/README.md`.


### Objetos Type criados pelo runtime

A tabela de `_Type` permanece ativa mesmo sem `AllocObject` local nem entrada no
vetor posicional de CIDs. O lowering publica a função da tabela e a premissa de
ancestralidade quando consegue resolver um pai distinto para o Type do SDK.
O teste compilado confere o CID e o payload escalar do construtor real do runtime,
além da identidade canônica sob GC. Essa retenção não autoriza chamadas nativas
opacas: o construtor RTI continua recusado pela prova de imagem fechada.
O caso de poda de plataforma passa dessa ancestralidade, mas ainda é rejeitado
pela checagem de namespace das entradas geradas de inicialização e callback.
A propriedade e o ABI dessas definições precisam ser comprovados sem abrir
exceções para símbolos reais do runtime. Evidências:
`bench/results/2026-10-19-runtime-type-tables/README.md`.

### Pai universal compartilhado e resumo versão 9

O CID 0 inserido pelo lowering é o pai universal terminal da relação de subtipos,
sem objetos próprios. Sua repetição entre chunks do SDK e a imagem local não
representa, por si só, uma colisão entre duas classes físicas. A emissão marca
esse papel com `; df.universal-parent 0`; o resumo versão 9 o conserva em `U\t0`.
Resumos de versões anteriores exigem reconstrução do cache.

A marca depende de uma única declaração sintética `Object` sem campos, tabela,
função de tabela, layout físico, identidade de programa, identidade criada pelo
runtime, ancestral próprio ou alocação HIR de CID 0. As premissas nominais e de
metadados do lowering também precisam estar presentes. O consumidor recusa
marcas duplicadas ou inconsistentes com os registros de classe, tabela, layout,
ancestralidade e alocação do resumo.

A reconciliação permite compartilhar somente a declaração nominal 0 quando o
SDK verificado e a imagem local satisfazem esse contrato. Tabelas e CIDs
ambíguos continuam colidindo; outros CIDs não recebem essa permissão. A marca
não autoriza alocar CID 0, executar inicializadores desconhecidos, acessar
layouts ou atravessar chamadas nativas não provadas. Essas obrigações continuam
independentes da identidade do pai universal.

### Entradas externas e endereços de pacotes

Uma entrada externa com parâmetro `ptr` pode receber um pacote de argumentos ou
seu descritor, cujo conteúdo não foi produzido pela imagem provada. Por isso a
prova fechada recusa tanto `Ref` quanto ponteiros nessas entradas, inclusive
raízes explícitas, callbacks e inicializadores. Entradas locais com inteiros,
booleanos, double e vetores sem referências continuam admitidas; isso não
certifica a ABI de um callback nativo específico.

A rejeição não pode ser contornada reinterpretando um inteiro ou valor gerenciado
como ponteiro. Casts de identidade `ptr` → `ptr` preservam um endereço existente;
unboxing precisa partir de `Ref` e produzir uma representação escalar com tipo de
resultado correspondente. Domínios não vazios do `Ref` de `df.call_dart0`
continuam exigindo prova própria. A análise de pacotes encaminhados e de
callbacks permanece separada da cobertura de alocações.


### Domínio vazio da entrada do loop

Na imagem fixa AOT, a prova pode testar a hipótese de que o loop nunca receberá
um callback. Para isso remove somente a aresta automática do emissor para a
entrada do loop em um resumo temporário. Chamadas reais, endereços publicados,
tabelas e referências do SDK continuam no grafo; se alcançarem a entrada por
outro caminho, a hipótese é recusada. A assinatura do protocolo continua sendo
`Ref (Ref)` e a rotina precisa existir localmente. Raízes controladas por código
nativo e trampolines FFI impedem esse caso-base, mesmo quando suas assinaturas
são escalares: a prova não cobre trabalho prévio ou ingresso externo nesse
contexto.

O conjunto de APIs nativas admitidas nessa prova não contém produtores de
trabalho Dart adiado. Microtasks, timers, dispatchers de portas/eventos nativos
e finalizadores não foram autorizados. Chamadas importadas precisam continuar
sem exposição e com fechamento de alocações vazio verificado. Portanto só a
cobertura completa da imagem, sem nenhuma dessas fronteiras abertas, demonstra
que o domínio é vazio; a ausência de sintaxe async ou de um import não basta.

Somente depois de todas as verificações a prova retira `call_dart`. A poda HIR
remove a rotina não referenciada e o emissor deixa de chamar o loop. Uma prova
que falha conserva a rotina e o loop originais. Isso não se aplica a imagens
abertas/JIT, nas quais a premissa de ancestralidade fixa está ausente, nem prova
um conjunto não vazio de callbacks. Admitir qualquer produtor de fila exige
uma prova adicional do domínio e da vida útil dos handles registrados.


### Operandos observados no inventário versão 2

O schema 2 de `allocation-inventory.json` preserva `arguments` em cada chamada
cuja ABI direta foi reconhecida. São operandos LLVM tipados, como `i64 300` ou
`ptr @"\01section$start$..."`, vinculados ao mesmo registro de corpo, módulo e
snapshot. O leitor confere contagem, representações e delimitadores no
subconjunto suportado; não transforma esses dados em certificado de registro,
alocação, origem de valor ou região de memória. Snapshots do schema 1 não
substituem observações ausentes e precisam ser reconstruídos.

O coletor reconhece aspas com escapes hexadecimais e delimitadores aninhados
nos argumentos; ponto e vírgula dentro de aspas não inicia comentário. Chamadas
indiretas, convenções/atributos não revisados e listas malformadas continuam
opacas. Isso torna visíveis registros de seções/imagens que antes apareciam
somente como ABI não suportada.

`i64` pode representar tanto inteiro não encaixotado quanto handle gerenciado.
Por isso um fechamento de alocações vazio e ABI compatível não bastam para
aceitar resultado `Ref`; retornar `ptr` também não demonstra um pacote ou sua
região. Esses resultados continuam sem autorização no consumidor de chamadas.
Resultados escalares e void conservam as regras de fechamento/exposição.
A validação dos dados de imagem estática e dos bindings dos helpers do SDK
continua necessária para autorizar suas rotinas de registro.

A extração do corpo também respeita aspas e comentários. Chaves em nomes de
parâmetros ou atributos textuais não delimitam uma função inline; o fechamento
multilinha pode ter comentário ao lado. Uma regressão com IR válido no LLVM
reproduziu o coletor ignorando a alocação real e publicando fechamento vazio.
O corpo completo agora conserva o CID alocado, inclusive nas funções que o
chamam, e o consumidor recusa a importação como fechamento vazio. O formato
não exigiu uma mudança de schema; o coletor participa do fingerprint do compilador
e invalida o cache SDK produzido pelo código anterior. O schema atual é descrito
abaixo.

### Startup nominal com certificado separado

A prova de classes aceita como raiz um startup `void ()` cujo corpo completo
publica relações literais por `dartforge_register_subclass` e/ou os metadados
textuais certificados abaixo. A função
deve ter o símbolo de registro do módulo, definição única e não interponível,
sem alocação direta, dependências executáveis ou obrigações adicionais. Cada
chamada exige a ABI exata, CIDs nominais válidos e não ambíguos, o filho declarado
no próprio módulo e a aresta presente no resumo verificado. CIDs compartilhados
entre módulos são recusados, salvo o pai universal 0 com seu certificado próprio.

O contrato foi conferido em `runtime/src/core.rs`: a publicação altera o mapa
de subtipos e invalida caches nativos, sem criar objetos Dart nem chamar helpers.
Isso não torna a função uma importação pura. Chamadas comuns ao startup e usos
diretos da API continuam sem autorização. Depois da reconciliação do snapshot,
uma cópia temporária do grafo elimina somente as arestas nativas pertencentes
aos startups certificados; os outros usuários da API permanecem visíveis. Os
resumos originais e os corpos executáveis não mudam.
Sem candidatos certificados, a prova reutiliza os resumos verificados sem cópia.

Registros de tabelas, imagens estáticas e helpers ainda exigem contratos próprios.
Os startups completos do SDK real continuam
recusados; este certificado não conclui a poda do explorador. A regressão
compilada conserva consultas nominais transitivas e inversas com registro amplo
e certificado, em execução normal e sob estresse de GC.

### Metadados textuais e inicializadores

O schema atual conserva `initializer` em todos os globais, vinculado
ao digest da linha original. O schema 3 guardava somente `native_initializer`
nas constantes de bytes; esse formato foi substituído pelo schema 4.
O leitor verifica digest, símbolo, propriedade local e inicializador literal
imutável. O decodificador confere o tamanho do array, escapes LLVM hexadecimais,
aspas, comentários fora de strings e alinhamento simples; formas não suportadas
continuam sem prova. Arrays gerenciados e tabelas de ponteiros não são textos.
Snapshots dos schemas anteriores precisam ser reconstruídos.

O certificado de entrada admite `dartforge_register_class_name` para CIDs
nominais locais válidos e `dartforge_register_argument_name` para nomes. As
chamadas exigem ABI exata, referência direta a uma constante privada única do
mesmo módulo, comprimento igual ao array completo e UTF-8 válido. Não são aceitos
ponteiros deslocados/GEP, regiões externas, comprimentos negativos/inexatos,
UTF-8 inválido ou outros efeitos nativos. Registros locais conflitantes de nome
para o mesmo CID são recusados.

Essas APIs copiam texto para mapas nativos, sem criar objetos Dart ou ativar
tabelas de classes. O teste do runtime confere os bytes UTF-8 e a estabilidade
do contador de alocações Dart. O teste compilado executa startup misto de nomes
e relações nominais, amplo/certificado, normal/com GC sob estresse. A função
continua sem autorização como chamada comum e as APIs não entram na lista
global de runtime coberto. O grafo temporário conserva somente referências reais
dos bytes literais, descartando citações espúrias de `@` dentro de seus textos;
o snapshot original é preservado.

### Observações dos blocos de strings estáticas no schema 4

Antes de autorizar uma faixa de imagem, é necessário conferir seus contribuidores
e os valores gerenciados que ela pode publicar. O inventário agora preserva
também os inicializadores gerenciados e recomputa sua classificação no leitor.
Um literal individual recebe `kind: static_string` somente quando coincide com
o layout permanente do emissor: objeto constante `linkonce_odr`, CID de string,
flags RAW, número de palavras, hash VM, comprimento e bytes preenchidos com zeros
até a palavra. A chave canônica é recalculada a partir do CID e das unidades.
UTF-16 contendo somente Latin-1 é recusado como representação não canônica.

O validador reconhece as grafias de seção do Mach-O, ELF e COFF e o alinhamento
exato de oito bytes. Isso observa o layout LLVM, sem provar os objetos finais
desses três formatos, relocação, comdat, âncoras, limites da seção ou o conjunto
completo de entradas do ligador. Âncoras e inicializadores não reconhecidos
permanecem separados dos literais conhecidos.

O diagnóstico de inventário informa o número de **definições** observadas e os
CIDs encontrados, antes de deduplicação pelo ligador. Não é contagem de objetos
únicos nem prova do universo inteiro da seção. Esses fatos não autorizam
`register_image`, exposição de handles estáticos ou retorno `Ref`/`ptr` de uma
importação. Os testes verificam explicitamente essa fronteira e a recusa de
classificações alteradas mesmo após recomputar a integridade da publicação.

### Censo de contribuições no IR — schema 5

Cada módulo conserva agora `image_census`, vinculado ao seu digest de IR final:
membros das seções de imagem, linhas canônicas dos marcadores e obrigações
abertas. O protocolo de marcadores vem da mesma função usada pelo emissor, com
formato explícito, sem depender do host para examinar Mach-O, ELF e COFF.

O scanner percorre todas as linhas, distingue símbolos/strings/comentários e
considera contribuições de funções e globais. Reconhece os literais já
validados e as âncoras zeradas exatas. Define obrigações para símbolos de
marcadores substituídos, declarações incompletas/duplicadas, formatos misturados,
globais multilinha, funções na seção e construções não revisadas. Assembly fica
aberto, salvo a diretiva exata do emissor que apenas retém os stackmaps e a
instrução ARM64 exata usada para ler o TSD no macOS. Essa instrução não muda
seções; sua string é compartilhada com o produtor. As leituras de memória e a
conversão para ponteiro que a seguem continuam sem prova de alocação/exposição.
Variantes do assembly não são aprovadas. Atributos
textuais de função também exigem revisão; `disable-tail-calls=true` é tratado
explicitamente. Strings com `section` e nomes entre aspas não viram diretivas.

O leitor reconcilia a parte global do censo com os inicializadores preservados,
os marcadores canônicos e o digest do módulo. Um registro sem obrigações informa
somente ausência de contribuições opacas no subconjunto de IR examinado pelo
produtor. Não verifica toda a gramática LLVM nem substitui a inspeção de objetos,
arquivos do runtime, bibliotecas estrangeiras, relocação, deduplicação ou opções
do ligador. As observações de corpos não são uma segunda cópia do IR completo;
continuam vinculadas à publicação do compilador e ao snapshot imutável.

Esse estado não entra nas condições de autorização: um censo de fonte limpo
continua insuficiente para `register_image`, retorno de handles ou ativação de
classes. A prova dos startups completos e da imagem ligada permanece pendente.


### Inspeção estrutural dos artefatos LLVM selecionados

`dartforge_llvm::inspect_image_sections` lê IR ou bitcode em um contexto próprio,
executa o verificador LLVM e devolve dados Rust independentes da vida do módulo.
Registra globais com seção explícita, declarações, funções com seção, assembly
inline/de módulo e nomes de aliases/ifuncs. Os inicializadores reconhecidos são
âncoras zeradas e estruturas não empacotadas de três palavras i64 seguidas de um
array i8. Ponteiros, poison e outros formatos permanecem não classificados.
Arrays zerados conservam o comprimento lógico sem alocar seu conteúdo.

A C API de seção não fornece comprimento. Para preservar inclusive NULs e evitar
confundir uma seção desconhecida com um prefixo conhecido, o leitor recupera o
atributo da impressão canônica do módulo e confere nomes e quantidade contra a
enumeração estrutural da C API. Uma única impressão evita o custo repetido de
imprimir cada valor. O decodificador aceita os escapes hexadecimais e a barra
invertida duplicada emitida pelo LLVM. Incompatibilidades de cabeçalhos encerram
a inspeção sem publicar observações parciais.

Com `embedded-llvm`, a integração do inventário com o SDK real também lê os bytes
dos artefatos selecionados e reconfere seus digests na publicação verificada.
Compara cada definição gerenciada retida com os inicializadores observados nas
fontes: símbolo, seção completa, conteúdo, alinhamento, endereço e propriedades
de armazenamento. O otimizador pode remover definições não usadas e restringir
a visibilidade; contribuições novas ou conteúdo alterado não são aceitos. A
checagem também recusa funções na seção, substituição por aliases/ifuncs e
assembly fora das formas exatas revisadas.

Essa comparação é anterior ao LTO. Os `.o` do SDK examinados são bitcode, não
uma certificação de seções nativas finais. Bibliotecas do runtime, entradas
estrangeiras, resoluções de marcadores, relocação, comdats e opções do ligador
continuam exigindo cobertura própria. O teste não altera as condições de
autorização de imagens/classes nem libera os startups completos do SDK.


### Entradas nativas e arquivos do runtime

`dartforge_emit_native::native_inputs::inspect` enumera objetos relocáveis
Mach-O/ELF/COFF e membros de bibliotecas autocontidas (`.a`/`.lib`), sem abrir caminhos externos
ou executar código do destino. Usa `object` com leitores desses formatos,
sem os recursos de escrita/descompressão. O resultado conserva ordem e nomes
em bytes, inclusive membros com nomes duplicados, digest da entrada e de cada
membro, faixas físicas, seções/segmentos, conteúdo armazenado e flags. Zonas
virtuais sem conteúdo não exigem alocação do tamanho lógico.

As seções candidatas de imagem e bitcode, `.drectve`, comandos Mach-O
`LC_LINKER_OPTION` e símbolos candidatos a marcadores ficam explícitos. Marcadores
conservam definição de endereço conhecido, estado indefinido/common, fraqueza,
escopo e classificação/índice de seção. Um símbolo common não se confunde com
uma referência indefinida só porque ainda não tem endereço conhecido.

Arquivos thin, membros aninhados/bitcode bruto, membros COFF de importação curta e
imagens não relocáveis são recusados. Falha num membro posterior não publica
inventário parcial. A leitura Mach-O percorre os comandos de carga explicitamente
com propagação de erros: o parser unificado tolerava uma iteração truncada que
poderia parecer um objeto sem seções. Isso não pretende verificar toda a gramática
de cada formato ou provar os efeitos de todas as diretivas/relocações.

Na rodada macOS ARM64, cada uma das três variantes do runtime tinha 466 membros.
Nenhuma seção nativa candidata à imagem ou símbolo de marcador foi observado.
Cada variante continha 229 seções candidatas a bitcode: 1.331.200 bytes ao todo.
As faixas/digests permitiram ler os 229 módulos embutidos da variante production
com a API LLVM verificada, sem contribuições explícitas de globais/funções à
seção de imagem. Cinco assemblies inline de `compiler_builtins` foram preservados
como não revisados; a ausência de assembly de módulo não os autoriza.

Essas observações não autorizam `register_image` nem mudam a poda. Ainda faltam
revisão do assembly, demais entradas estrangeiras, seleção dos membros pelo
ligador, resolução dos marcadores, opções/diretivas, relocação, LTO e imagem
final. Objetos para cinco destinos foram inspecionados em fixtures; essa rodada
não executa os sistemas estrangeiros nem inspeciona os runtimes Windows/Linux.
A nova dependência atualiza `Cargo.lock`, que participa do hash de fontes do
runtime: bundles antigos precisam ser reexportados, embora o digest do arquivo
production local tenha permanecido igual ao da rodada anterior.


### Leitura das seções de strings no arquivo ligado

`dartforge_emit_native::linked_images::inspect` lê imagens Mach-O, ELF e PE
64-bit little-endian para ARM64/x64. Exige uma única seção reconhecida
(`__DATA_CONST,__dfimg`, `dfimg` ou `.dfimg`), alinhada e inteiramente armazenada
no arquivo. O endereço observado é o endereço virtual preferido do arquivo,
não um ponteiro de um processo em execução.

O percurso cobre todos os bytes da região como palavras zeradas ou blocos de
string. Palavras zeradas são observadas como padding; não recebem identidade
certificada de âncora. Cada bloco exige header permanente RAW exato, CID de
string, comprimento limitado pelos bytes disponíveis, hash VM, codificação
Latin-1/UTF-16 canônica e padding zerado. Surrogates isolados são válidos. O número
de palavras saturado do header não substitui o comprimento real para delimitar
strings grandes. Truncamento, classes/formas não revisadas e conteúdo alterado
encerram a leitura sem publicar resultado parcial.

A validação dos bytes é compartilhada com o inventário de inicializadores LLVM;
a análise textual continua exigindo símbolo/chave canônica, seção e propriedades
de armazenamento. O núcleo devolve um digest de tamanho fixo, sem introduzir uma
alocação de String para cada inicializador validado nas fontes. A alocação da
chave em String só ocorre ao publicar os registros da leitura final. A enumeração fallível
de comandos Mach-O é compartilhada com o leitor de entradas nativas.

Foram lidos os três exploradores já registrados na matriz desktop: macOS ARM64
(14.396 strings, 850.904 bytes de seção), Linux ARM64 (14.548, 854.760 bytes) e
Windows x64 (14.534, 871.040 bytes). Os SHA-256 dos arquivos coincidem com a
publicação anterior. Todas as strings observadas têm CID 6/7; cada arquivo tem
36 strings de dois bytes. Isso não é nova execução ou recompilação do explorador.
Fixtures C de layout foram compiladas/ligadas novamente para os cinco destinos;
modificar o hash de uma string no arquivo final foi recusado em todos eles.

Esses fatos ainda não autorizam registro de imagem ou poda: a leitura observa
bytes do arquivo, enquanto rebases/binds/relocações do carregador podem modificar
a memória. Permissões, limites realmente passados a `register_image`, resolução
de marcadores, opções/entradas do ligador e proveniência das contribuições
continuam exigindo cobertura própria. Não houve alteração dos gates de classes
ou do schema do inventário, nem ganho de tamanho/desempenho comprovado.


### Auditoria declarativa de relocações ELF

`linked_images::elf_relocations::inspect` lê ELF64 little-endian ARM64/x64 e
parte das referências de `PT_DYNAMIC`, não dos nomes de seções de relocação.
Isso evita concluir ausência de relocações quando o iterador genérico de seções
não as encontra. O leitor exige cabeçalhos suportados, mapeamento único dos
bytes de metadados por `PT_LOAD`, dados dinâmicos terminados por `DT_NULL` e
consistência de tamanhos/entradas. A seção de strings é validada novamente e
seu digest confere com os bytes do mapeamento declarado.

São decodificadas as tabelas REL, RELA, JMPREL e RELR. O teste de sobreposição
usa toda a largura potencial da escrita, incluindo uma escrita que começa
antes da seção e a atravessa. Tipos não revisados e COPY ficam sem largura
conhecida, em contadores separados; tags dinâmicas desconhecidas permanecem
explícitas. Não há conversão de ausência de classificação em escrita segura.
Os prefixos rápidos RELCOUNT/RELACOUNT exigem entradas RELATIVE do destino e
símbolo zero, para não interpretar como inofensivo um NONE no prefixo.

RELR é percorrido sem criar uma lista expandida de endereços. Bitmaps exigem
endereço anterior, cursores e faixas usam aritmética verificada e endereços
pares não alinhados a palavra são preservados. Cada tabela tem contadores de
todas as entradas/escritas e no máximo 16 exemplos por categoria; limitar
exemplos não interrompe a análise. Metadados de tamanho de entrada sem tabela
principal também são aceitos para imagens que possuem apenas relocações PLT.

O explorador Linux ARM64 já publicado apresentou 67.445 entradas RELA
(67.434 RELATIVE e 11 GLOB_DAT) e 133 JMPREL/JUMP_SLOT: 67.578 alvos declarados,
sem sobreposição conhecida com os 854.760 bytes de strings e sem larguras/tags
não revisadas nessa amostra. O mapeamento declarado da região tem flags 4
(leitura); isso não certifica permissões reais. Há tags de inicialização.
Fixtures PIE novas para ARM64/x64 passaram com RELA e RELR compactado. Alterar
um alvo para a seção de strings foi detectado nos quatro casos, sem executá-los.

A auditoria descreve registros do arquivo. Não prova validade de todos os alvos,
estabilidade das próprias tabelas durante o carregamento, escritas implícitas
como GOT/DT_DEBUG, código chamado por resolução de símbolos/IFUNC, inicializadores,
dependências, permissões reais ou limites registrados. Não há certificado de
imutabilidade, gate automático de link ou autorização nova de classes/imagens.
Mach-O/PE ainda precisam da análise dos respectivos mecanismos de fixup.

Referências normativas: [gABI — estrutura dinâmica](https://gabi.xinuos.com/elf/08-dynamic.html),
[gABI — REL/RELA/RELR](https://gabi.xinuos.com/elf/06-reloc.html),
[AAELF64](https://github.com/ARM-software/abi-aa/blob/main/aaelf64/aaelf64.rst) e
[AMD64 psABI](https://refspecs.linuxfoundation.org/elf/x86_64-abi-0.99.pdf).

## Registro único da imagem do bundle SDK (20/10/2026)

O caminho privado que monta o bundle completo do SDK delega o registro da imagem
à primeira raiz de inicialização, mantendo a ordem fixa de `SOURCE_LIBRARIES`.
As outras 14 raízes preservam todos os demais registros e os marcadores/objetos
estáticos. A emissão pública por biblioteca continua independente, com registro
próprio; o programa conserva seu registro porque pode carregar o SDK em outra
imagem. Não há mudança da ABI do runtime nem nova autorização de poda.

A remoção dessas chamadas efetivamente redundantes permite às regras anteriores
certificarem sete entradas apenas nominais/textuais; a entrada inicial com imagem
mais helpers continua aberta. Os testes exigem o conjunto exato de entradas e
um único proprietário no inventário selecionado. Evidências e limites:
[proprietário da imagem SDK](../bench/results/2026-10-20-sdk-image-owner/README.md).
