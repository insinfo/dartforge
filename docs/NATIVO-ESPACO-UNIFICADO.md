# Espaço unificado: todos os valores do runtime no espaço de objetos

Especificação das fases B, C e D da proposta estrutural de `PLANO-TAMANHO-DESEMPENHO.md` §2,
para implementação em paralelo e teste no fim. Escrita em 2026-09-30 sobre o working tree de
`d1878f01` (com as mudanças não commitadas das frentes de tamanho, classe do valor e vetorização).
Os números de linha citados são desse estado; `RT/` é `crates/runtime/src/`, `EN/` é
`crates/emit_native/src/`, `VM/` é `E:\references\dart-sdk\` (commit `6c4009687d2c`) e `JL/` é
`E:\references\julia\src\`.

Esta especificação **substitui** o desenho incremental da fase B (strings primeiro, com os slots
ainda vivos). A decisão do proprietário é o desenho final certo de uma vez, com quebra
temporária aceita.

## 0. Resumo

Hoje o runtime tem **dois heaps no mesmo isolado**:

* o **espaço de objetos** (`RT/heap.rs` `EspacoDeObjetos`, 1730-1792): objetos das classes do
  programa e do SDK compilado da fonte, com cabeçalho de 16 bytes, alocação em linha por TLAB,
  barreira de escrita e coleta geracional sem mover por mapa de marcas;
* a **tabela de slots** (`Heap::slots: Vec<Option<Value>>`, `RT/heap.rs:2921`): strings,
  `StringBuffer`, caixas de `int`/`double`/`bool`, células, ambientes, closures, `_List`,
  `_GrowableList`, `_ImmutableList`, records posicionais, listas tipadas, visões, valores SIMD e
  o legado sem SDK (`Map`, `Set`, `RegExp`, `Match`), cada um com o conteúdo num `Vec` do
  `malloc`, seis vetores paralelos por slot e um segundo coletor.

Depois desta especificação só existe o espaço de objetos. Cada valor do runtime é um bloco com o
mesmo cabeçalho, uma classe (`cid`) **fixa**, conhecida pelo runtime e pelo emissor ao compilar, e
um de três formatos de corpo que o coletor sabe percorrer sem olhar a classe:

| Formato | O coletor | Quem usa |
|---|---|---|
| `INSTANCIA` | segue os campos cujo bit está aceso no mapa (o de hoje) | objetos do programa e do SDK, `_GrowableList`, `_Closure`, contextos, células, visões tipadas |
| `BRUTO` | não segue nada | strings, `_Mint`, `_Double`, `bool`, SIMD, listas tipadas internas, `_List` compacta de `int`/`double`/`bool`, anexos nativos |
| `REFS` | segue toda palavra par e não nula depois do comprimento | `_List`/`_ImmutableList` geral, `_Record` |

O que muda, em uma frase por eixo:

1. **Handle:** `0` null, ímpar `Smi`, `bloco + 2` objeto. Somem os handles múltiplos de 4.
2. **Cabeçalho:** o mesmo de hoje (16 B), com o formato do corpo, cartões, anexo, forma compacta e
   memória externa em `flags`, o estado `PERMANENTE` para objetos estáticos e o hash da string em
   `mapa`.
3. **Classes:** cids 1–65 fixos para as classes que o runtime conhece (`_OneByteString` = 6,
   `_List` = 8…); o programa começa em 128. `df.classe` vira três casos sem chamada.
4. **Alocação:** as classes de tamanho vão de 0–64 palavras exatas para mais 24 classes médias até
   2014 palavras (16 KiB); acima disso, uma região grande própria (`VirtualAlloc`/`mmap`) com
   granularidade de 4 KiB. A alocação em linha serve a qualquer objeto de até 16 palavras,
   inclusive strings e caixas de tamanho dinâmico.
5. **Barreira:** a de hoje, mais o teste do filho jovem, e cartões de 32 elementos nas `_List`
   grandes (o `CardRemembered` da VM).
6. **Coletor:** um só. A marcação despacha pelo formato; a varredura continua lendo só os bits.
   Os anexos nativos (acumulador do `StringBuffer`, programa de `RegExp`) são soltos quando o
   dono morre.
7. **Literais de string** no AOT são objetos estáticos (`PERMANENTE`) no executável, sem
   chamada nem cache no ponto de uso; o JIT continua internando no heap.
8. **Fixa e imutável** são classes (`_List`, `_ImmutableList`), escolhidas na alocação; o `cid`
   nunca muda depois de publicado.
9. **Some:** `enum Value`, `Heap::slots` e os seis vetores por slot, `TaggedValue` no heap,
   `ClasseDoSlot`/`Contexto::classes`, os cabeçalhos de endereço fixo (`CabecalhoDeLista`,
   `CabecalhoTipado`, `CabecalhoDeClosure`), 35 externs de acesso à representação e o caminho sem
   SDK da fonte (`DARTFORGE_SDK_DA_FONTE=0`, `lower/sdk_por_nome.rs`, `colecoes.rs` legado e
   cerca de 75 externs).

A divisão em pacotes (§4) é: **P0** fundação (contrato em ~1 dia, implementação em paralelo),
**P1** strings, **P2** caixas/células/contextos/closures/records, **P3** listas/mapas/conjuntos,
**P4** listas tipadas/SIMD/FFI/E/S, **P5** transversais (isolados, RTI, erros, saída) e remoção
do legado. Cada arquivo tem um dono só.

---

## 1. Inventário do estado atual

### 1.1 Como um handle é etiquetado hoje

| Forma do `i64` | Significado | Onde |
|---|---|---|
| `0` | null | `RT/heap.rs:2711-2713` (`smi::e_handle`), checagens em `indice_vivo` (3962) e `key_equal` (4139) |
| ímpar | `Smi`: `(v << 1) \| 1`, `v ∈ [-2^62, 2^62)` | módulo `smi`, `RT/heap.rs:2686-2714` (`MIN`/`MAX` 2688-2689, `de` 2693, `e_smi` 2699, `valor` 2705); o coletor pula ímpares (4473, 4512) |
| `h & (3 \| MIN) == 2` | objeto do espaço, `h = bloco + 2` | `e_objeto` (`RT/heap.rs:2665`), `DESLOCAMENTO_DO_HANDLE` (1154), `bloco_de` (2135-2143), `bloco_vivo` (3789-3797) |
| `h > 0`, `h & 3 == 0` | slot: índice `(h >> 2) - 1` | `handle_de_indice` (3939-3944), `indice_de` (3946-3948), `indice_vivo` (3960-3983, cinco pânicos N4); fora do heap: `RT/nucleo.rs:699`, `RT/nativos_listas.rs:518` |
| negativo | nunca handle (ids negativos só como chave de `globais`: `RT/eventos.rs:23`, `RT/tipos.rs:1401`) | `marcado_na_coleta` 2635, `try_get` 4092 |

No emissor a mesma divisão está em `@df.classe` (`EN/llvm/mod.rs:3070-3104`: `h==0`, `h&1`,
`h & 0x8000000000000003 == 2`, `== 0` para slot) e na leitura de campos
(`emitir_endereco_dos_campos`, `EN/llvm/mod.rs:2532-2550`, que troca o bloco por `ctx+40`,
`OBJETO_VAZIO`, quando não é objeto).

*Achado:* a documentação do módulo `smi` (`RT/heap.rs:2677`) ainda diz que o handle de slot é
`(índice + 1) << 1`; o código usa `<< 2` (3942).

### 1.2 O `enum Value` (`RT/heap.rs:2364-2414`) e quem cria e lê cada variante

Pontos por onde toda variante passa: `Value::estimated_bytes` (2550-2574), `Value::trace`
(2579-2627), `Heap::allocate` (3479), `Heap::allocate_linked` (4105-4125), `classe_calculada`
(3579-3606), `guardar` (3529-3554).

| Variante | Payload | Cria (fora de `heap.rs`) | Lê / casa (fora de `heap.rs`) |
|---|---|---|---|
| `String(Texto)` | `Um(Vec<u8>)`/`Dois(Vec<u16>)` (117-121) | `strings.rs:13` (`alocar_texto`, base de `alocar_str` e de todos os natives), `closures.rs:198`, `colecoes.rs:207,213,527`, `despacho.rs:102,112`, `excecoes.rs:29,61,81,319,494,507,640`, `nativos_listas.rs:53,63`, `nativos_strings.rs:279`, `portas.rs:537`, `saida.rs:546,574,580,587,595,604`, `strings.rs:516,592`, `tipos.rs:1374,1412`; indiretos: `string_literal` (`nativos_listas.rs:1112`, `nativos_strings.rs:81`, `strings.rs:102`), `alocar_str` em 17 arquivos (io_soquetes 10, io_plataforma 9, tls 9, …) | `closures.rs:287,320`, `colecoes.rs:196,236,472,504`, `despacho.rs:81`, `excecoes.rs:41,417`, `io_processos.rs:49`, `isolados.rs:280`, `nativos_hash.rs:56,87`, `nativos_strings.rs:25-26,35,49-50,159,171,326-327`, `nucleo.rs:91,736`, `portas.rs:107,179`, `saida.rs` (17 linhas: 94…599), `seletores.rs:265`, `strings.rs:37,47,263,277,286,395,671`, `tipos.rs:162,815` |
| `StringBuffer(Vec<u16>)` | acumulador | `nativos_strings.rs:290`, `strings.rs:659`, `portas.rs:538` | `nativos_strings.rs:301,308,330,348,364`, `strings.rs:48,264,278,287,675`, `colecoes.rs:473`, `nucleo.rs:737`, `portas.rs:180`, `saida.rs:159`, `tipos.rs:816` |
| `RegExp(Texto)` | padrão (legado) | `strings.rs:553`, `portas.rs:539` | `nucleo.rs:746`, `portas.rs:181`, `saida.rs:156`, `strings.rs:396`, `tipos.rs:817` |
| `Match(Texto)` | texto (legado) | `strings.rs:592`, `portas.rs:540` | `colecoes.rs:209,236`, `strings.rs:47,263,277,286`, … |
| `Objeto` | marcador, nunca em slot | `static VALOR_OBJETO` (2659), `get_mut` (4427) | `nucleo.rs:734`, `portas.rs:183,586,587`, `saida.rs:116,232`, `tipos.rs:163,824` |
| `Cell(TaggedValue)` | captura mutável | `create_cell` (4153) ← `closures.rs:7`; `portas.rs:542,619` | `cell_get`/`cell_set` (4158/4166), `portas.rs:208`, `saida.rs:208`, `tipos.rs:826` |
| `Environment(Vec<TaggedValue>)` | capturas | `create_environment` (4174) ← `closures.rs:46`, `ffi.rs:183`, `tearoff` (3402); `portas.rs:543,623` | `environment_get` (4179), `closures.rs:149` (`dartforge_env_dados` expõe o `Vec`), `nativos_listas.rs:838,1065`, `seletores.rs:423`, `portas.rs:209`, `saida.rs:207` |
| `Closure(Box<CabecalhoDeClosure>)` | código, ambiente, corpo tipado, ABI | `create_closure` (4191) ← `closures.rs:62,72`, `ffi.rs:186`, `tearoff` (3404); `closures.rs:89`; `portas.rs:545,628` | `closure_parts` (4196), `closures.rs:73,104,125,137,182`, `nativos_listas.rs:836,1063`, `nucleo.rs:741`, `seletores.rs:281`, `portas.rs:210`, `saida.rs:206`, `tipos.rs:822` |
| `List(Elementos)` | `_List`/`_GrowableList`/`_ImmutableList` pelas marcas do slot | `create_list` (4203) ← `colecoes.rs:122,128,415,547`, `despacho.rs:164`, `ffi_callbacks.rs:575`, `nativos_listas.rs:1120`, `strings.rs:303,441`; `allocate` direto em `closures.rs:339,345`, `colecoes.rs:486,493,597`, `io_arquivos.rs:190`, `nativos_listas.rs:207,262,279,300,630,648,972`, `portas.rs:552` | 30 linhas em `nativos_listas.rs` (73…646, 9 delas `get_mut`), 14 em `colecoes.rs`, `despacho.rs:152,277`, `excecoes.rs:416`, `ffi_callbacks.rs:533`, `isolados.rs:170`, `nativos_hash.rs:109`, `nativos_strings.rs:383`, `nucleo.rs:738`, `portas.rs:174,216,635`, `saida.rs:108,162`, `seletores.rs:272`, `strings.rs:373`, `tipos.rs:819,930,935`, `zlib.rs:199` |
| `Map(Vec<(TV,TV)>)` | só sem SDK da fonte | `create_map` (4285) ← `colecoes.rs:295`; `portas.rs:553,648` | `colecoes.rs:395,398,412,475`, `nucleo.rs:739`, `portas.rs:217`, `saida.rs:111,176`, `tipos.rs:820` |
| `Set(Vec<TV>)` | só sem SDK da fonte | `create_set` (4353) ← `colecoes.rs:437`; `portas.rs:554,655` | `colecoes.rs:476`, `despacho.rs:278`, `nucleo.rs:740`, `portas.rs:218`, `saida.rs:115,192`, `tipos.rs:821` |
| `Record(Vec<TV>)` | record só posicional | `nucleo.rs:766` (`dartforge_record_new`); `portas.rs:555,662` | `despacho.rs:253,263`, `nativos_listas.rs:956,982`, `nucleo.rs:745`, `seletores.rs:285`, `portas.rs:219`, `saida.rs:215`, `tipos.rs:823` |
| `BoxedInt(i64)` | `_Mint` | `caixa_int` (3232) ← `despacho.rs:87`, `io_arquivos.rs:164`, `nucleo.rs:604`; `como_ref` (3217) ← `isolados.rs:177`, `nativos_hash.rs:238,302`, `nucleo.rs:671`, `portas.rs:682`, `seletores.rs:438` | `int_de_ref` (3241), `normalizar` (3257), `nativos_hash.rs:55,73`, `nucleo.rs:544,742`, `saida.rs:112,212`, `seletores.rs:282`, `tipos.rs:159,812` |
| `BoxedDouble(f64)` | `_Double` | `como_ref` (3221), `despacho.rs:91`, `nucleo.rs:610`, `nativos_listas.rs:902`, `portas.rs:557` | `despacho.rs:70`, `ffi_callbacks.rs:285`, `nativos_hash.rs:74`, `nativos_listas.rs:591`, `nucleo.rs:545,644,743`, `saida.rs:113,213`, `seletores.rs:283`, `tipos.rs:160,813` |
| `BoxedBool(bool)` | dois singletons (`caixas_bool`, 2981) | `caixa_bool` (3198) ← `despacho.rs:95`, `io_arquivos.rs:160`, `nucleo.rs:616`, `portas.rs:579` | `ffi_callbacks.rs:287`, `io_soquetes.rs:413`, `isolados.rs:239`, `nucleo.rs:546,657,744`, `saida.rs:114,214`, `seletores.rs:284`, `tipos.rs:161,814` |
| `TypedData{tipo,class_id,bytes}` | listas tipadas internas e SIMD | `typed_data.rs:94`, `simd.rs:21`, `ffi.rs:794,844`, `io_arquivos.rs:176,183`, `portas.rs:558,559,564` | `typed_data.rs:49,58,66,158,180,260`, `tls.rs:766,776` (`mem::take` do `Armazenamento`), `nativos_hash.rs:101,163`, `simd.rs:28`, `ffi_callbacks.rs:564`, `nucleo.rs:705,735`, `saida.rs:226`, `tipos.rs:825`, `portas.rs:223` |
| `TypedView{…}` | visões | `typed_data.rs:114`, `portas.rs:570,666` | `typed_data.rs:50,159`, `nucleo.rs:706,735`, `saida.rs:226`, `tipos.rs:825`, `portas.rs:224` |

Contagem de linhas com `Value::X` por arquivo (produção; criações / leituras):

| Arquivo | Cria | Lê | Arquivo | Cria | Lê |
|---|---:|---:|---|---:|---:|
| `heap.rs` | 12 (+32 em testes) | 41 + 18 braços (+13 em testes) | `nativos_strings.rs` | 2 | 15 |
| `saida.rs` | 6 | 37 | `strings.rs` | 5 | 14 |
| `nativos_listas.rs` | 10 | 32 | `closures.rs` | 7 | 8 |
| `colecoes.rs` | 6 | 25 | `seletores.rs` | 0 | 8 |
| `tipos.rs` | 2 | 22 | `nativos_hash.rs` | 0 | 8 |
| `nucleo.rs` | 2 | 21 | `typed_data.rs` | 2 | 8 |
| `portas.rs` | 19 + 5 substituições + 3 in-place | 21 | `despacho.rs` | 3 | 7 |
| `excecoes.rs` | 7 | 3 | `ffi_callbacks.rs` | 0 | 4 |
| `io_arquivos.rs` | 3 | 0 | `isolados.rs` | 0 | 3 |
| `ffi.rs` | 2 | 0 | `tls.rs` | 0 | 2 |
| `simd.rs` | 1 | 1 | `io_processos.rs`, `io_soquetes.rs`, `zlib.rs` | 0 | 1 cada |

Não citam `Value::` (usam só auxiliares como `alocar_str`, `dart_bytes`, `dart_int`, `dart_bool`,
`dart_lista_fixa`, `texto_de`): `alocador`, `compat_jit`, `eventos`, `finalizadores`, `gc_raizes`,
`hash`, `io_diretorios`, `io_eventos`, `io_observador`, `io_plataforma`, `io_servico`,
`io_soquetes_unix`, `io_windows_*`, `nativos_desenvolvedor`, `nativos_numeros`,
`nativos_sistema`, `regexp`, `tls_formatos`, `ffi_api_nativa`. `crates/runtime/tests/*.rs` só
usam a ABI (`tests/tipos_e_eventos.rs:12,28-29`).

Auxiliares de leitura que concentram o acesso (nº de chamadas, com a definição):
`texto_de` (strings 19, nativos_listas 3, regexp 1), `com_texto` (nativos_strings 4,
nativos_listas 3), `texto_de_qualquer` (strings 8), `padrao_de` (strings 8), `resolver`
(typed_data 11, io_arquivos 2, ffi, nativos_listas, simd), `bytes_de` (typed_data 5),
`bytes_de_mut` (typed_data 4, tls 2), `simd_bytes` (simd 9), `lista_len` (nativos_listas 8,
closures 2), `chave_do_valor` (tipos 5), `tipo_do_valor` (tipos 4), `cid_do_runtime`
(nativos_listas 7, tipos 2, …), `dart_bytes` (io_soquetes 10, …), `dart_lista_fixa`
(io_soquetes 9, io_observador 8, seletores 4, …), `dart_int` (io_soquetes 17, io_arquivos 9, …).

### 1.3 Chamadas à API do `Heap` por arquivo

| Arquivo | Chamadas |
|---|---|
| `heap.rs` (produção) | get 22, try_get 4, get_mut 5, texto 2, objeto 4, allocate 8, normalizar 15, key_equal 7, string_equal 1, classe_do_objeto 1, definir_campo 2, novo_objeto 1, alocar_objeto 1 |
| `nativos_listas.rs` | get 10, try_get 16, get_mut 7, allocate 10, list_get 5, list_set 1, list_push 2, string_equal 2, metadado 2, set_metadado 1, marcar_fixa 3, e_fixa 1, marcar_imutavel 3, e_imutavel 2, normalizar 5, int_de_ref 2 |
| `colecoes.rs` | get 15, try_get 1, get_mut 2, allocate 6, list_* 8, map_* 12, set_* 3, key_equal 1, marcar_imutavel 1, e_imutavel 1, normalizar 1 |
| `saida.rs` | get 2, try_get 15, texto 1, objeto 1, classe_do_objeto 1, allocate 6 |
| `portas.rs` | get 2, try_get 1, get_mut 8, objeto 2, classe_do_objeto 3, definir_campo 2, novo_objeto 2, allocate 1, metadado 1, set_metadado 1, marcar_fixa 1, e_fixa 1, marcar_imutavel 3, e_imutavel 1, e_permanente 1 |
| `nativos_strings.rs` | get 1, try_get 5, get_mut 6, texto 10, allocate 2, list_push 1, hash_de_texto 1 |
| `strings.rs` | get 4, try_get 4, get_mut 1, texto 4, allocate 4, list_push 2, string_equal 1 |
| `excecoes.rs` | try_get 3, texto 2, objeto 8, definir_campo 1, novo_objeto 6, allocate 7 |
| `closures.rs` | try_get 7, get_mut 1, allocate 7, cell_get 3, cell_set 1, closure_parts 2 |
| `tipos.rs` | try_get 1, get_mut 1, texto 4, objeto 1, classe_do_objeto 2, novo_objeto 1, allocate 2, metadado 1, set_metadado 2, metadado_e_valor 2 |
| `nucleo.rs` | get 1, try_get 5, objeto 2, classe_do_objeto 2, alocar_objeto 1, allocate 2, string_equal 1, classe_do_slot 1, normalizar 1, int_de_ref 1 |
| `ffi.rs` | texto 3, objeto 4, definir_campo 2, allocate 2, environment_get 2, closure_parts 1, set_metadado 2, int_de_ref 2 |
| `despacho.rs` | get 1, try_get 6, allocate 1, metadado 1, set_metadado 1, int_de_ref 1 |
| `nativos_hash.rs` | try_get 6, get_mut 1, list_set 4, hash_de_texto 1 |
| `io_arquivos.rs` | texto 1, objeto 2, definir_campo 1, allocate 3, list_set 1, marcar_fixa 1, int_de_ref 3 |
| `io_soquetes.rs` | try_get 1, texto 3, list_len 1, list_get 2, int_de_ref 2 |
| `isolados.rs` | try_get 3, texto 2, objeto 1, list_len 1, list_get 1, int_de_ref 1 |
| `typed_data.rs` | get 1, try_get 3, get_mut 2, allocate 2 |
| `seletores.rs` | get 1, try_get 1, texto 2, e_fixa 1, e_imutavel 1 |
| `ffi_callbacks.rs` | try_get 3, closure_parts 1, int_de_ref 2 |
| `io_plataforma.rs` | list_len 1, list_get 1, list_set 1, set_metadado 1 |
| `io_processos.rs` | try_get 1, list_len 1, list_get 1 |
| `tls.rs` | try_get 2 |
| `simd.rs` | get 1, allocate 1 |
| `zlib.rs` | try_get 1, int_de_ref 1 |
| `compat_jit.rs` | texto 1, objeto 1 |
| `ffi_api_nativa.rs` | texto 1 |
| `gc_raizes.rs` | marcar_permanente 1, marcar_constante 1 |

Os testes de `heap.rs` (módulo `#[cfg(test)]` a partir de 4931) somam get 14, allocate 32,
alocar_objeto 21, objeto 17, novo_objeto 11, classe_do_slot 7, map_contains 7 e o resto da API
de coleções: todos serão reescritos (P0).

### 1.4 Estruturas de endereço fixo que o código gerado lê em linha

Todas existem para contornar o fato de o `Value` morar num `Vec` que realoca: um `Box` de endereço
fixo e uma função "pura" do handle que o LLVM tira dos laços.

| Estrutura | Layout | Obtida por | Lida em |
|---|---|---|---|
| `CabecalhoDeLista` (`RT/heap.rs:634-667`, vazio 671) | `dados`@0, `len`@8, `gravavel`@16 (bits `1<<código` e `1<<(código+4)`), `forma`@24, `vetor`, `compacto`, `logico` | `dartforge_lista_cabecalho` (`RT/nativos_listas.rs:167`; decl `EN/llvm/externs.rs:391`, `memory(none) … speculatable`) | `EN/lower/tipados.rs:297,308-310,332-350,734,765,811-827,854-865`; `EN/lower/comandos.rs:322-344`; `EN/lower/sdk_fonte.rs:2128-2139` |
| elementos gerais `TaggedValue` (`RT/heap.rs:51-62`) | 16 B: bits@0, `is_ref`@8, tag@9 (Int 0, Bool 1, Double 2, Ref 3) | `dados + 16i` | `EN/lower/tipados.rs:811-818`; espelho da tag em `tipados.rs:880-883` |
| `CabecalhoTipado` (`RT/heap.rs:2427-2446`, vazio 2451) | `dados`@0, `bytes`@8, `proprio: Vec<u8>`, `externo` | `dartforge_typed_cabecalho` (`RT/typed_data.rs:178`; `EN/llvm/externs.rs:346`) | `EN/lower/tipados.rs:466-568` (cache por ponto, `typed_cabecalho_na_falha` 561); `!invariant.load` em `EN/llvm/mod.rs:990-1021` |
| `CabecalhoDeClosure` (`RT/heap.rs:1058-1073`, vazia 1077) | `code_id`@0, `environment`@8, `tipado`@16, `abi`@24 | `dartforge_closure_cabecalho` (`RT/closures.rs:135-140`; `externs.rs:407`) | `EN/lower/closures.rs:794-822` |
| vetor do `Environment` | `Vec<TaggedValue>` | `dartforge_env_dados` (`RT/closures.rs:147-152`; `externs.rs:413`) | `EN/llvm/mod.rs:1495-1506` e `@df.env_ref` (3163-3184) |
| unidades do `Texto` | `Vec<u8>`/`Vec<u16>`, bit 63 = dois bytes | `dartforge_texto_dados`/`_len`/`_na_falha` (`RT/nativos_strings.rs:22-56`; `externs.rs:361-372`) | `EN/lower/textos.rs:92-188` (cache por valor lido) |
| `ClasseDoSlot` (`RT/heap.rs:2803-2828`) | `classe: i32`, `marcas: u32` por slot | `Contexto.classes`/`n_classes` (+336/+344) | `@df.classe`, `EN/llvm/mod.rs:3081-3100` |

O `tls.rs` depende do `Box` de forma ainda mais forte: `tomar_bytes_tipados` (763-771) faz
`std::mem::take` do `Armazenamento` durante o processamento síncrono e `devolver_bytes_tipados`
(773) o recoloca.

### 1.5 Tabelas laterais do `Heap` (`RT/heap.rs:2920-3086`)

| Campo | Tipo | Guarda | Purgado na coleta |
|---|---|---|---|
| `slots` (2921) | `Vec<Option<Value>>` | os valores | varredura dos slots (4778-4813) |
| `metadados` (2925) | `Vec<i64>` | RTI do slot (`id+1`) | zerado no reuso |
| `free` (2926) | `Vec<usize>` | slots livres | alimentado pela varredura |
| `marks` (2932), `idade` (3031), `slots_jovens` (3034), `slots_lembrados` (3035) | | marca, geração e barreira dos slots | a cada coleta |
| `bytes_do_slot` (3005) | `Vec<u32>` | bytes estimados por slot | regravado |
| `hashes_de_texto` (3042) | `Vec<Cell<u32>>` | `hashCode` de `String` | zerado no reuso e em `get_mut` (4431) |
| `classes` (2996), `cids_do_runtime` (3000) | | classe do slot (publicada no `Contexto`) | 4784, 4809 |
| `frames` (2927) | `Vec<(i64, Vec<i64>)>` | quadros de raízes do runtime (`com_raizes`, `RT/gc_raizes.rs:8-20`) | são raízes |
| `literais` (2958) | `HashMap<Vec<u16>, i64>` | literais canônicos | raiz permanente (4576) |
| `permanentes` (2964), `constantes` (2969), `codigo_do_tearoff` (2971) | | identidade entre portas, getter da constante | **nem raiz nem purgados** (achado: podem guardar handle morto se a constante não estiver também num global) |
| `tearoffs` (2955), `enum_values` (2949), `globais` (2979), `raizes_do_runtime` (2984), `caixas_bool` (2981), `finalizacoes_prontas` (3085) | | raízes | não |
| `imutaveis_do_espaco` (2991) | `HashSet<i64>` | objetos do espaço imutáveis | 4721-4722 |
| `campos_late_inicializados` (3008), `late_novos` (3012) | | campos `late` escritos | 4728-4735 |
| `iteracoes_ativas` (3059), `origens` (3062) | | legado de `colecoes.rs` | 4737-4741 |
| `finalizaveis` (3068), `fracas` (3072), `efemeros` (3076), `anexos` (3082) | | `Dart_NewFinalizableHandle`, `WeakReference`, `_WeakProperty`, `Finalizer` | 4705-4766 (ponto fixo dos efêmeros 4683-4698) |

### 1.6 O coletor e o espaço de objetos hoje

* **Cabeçalho** (`RT/heap.rs:1108-1134`): `estado` u8 @0 (`LIVRE`, `JOVEM`, `MARCADO`, `VELHO`,
  `LEMBRADO`, 1145-1149), `flags` u8 @1 (`FORA` = 1, 1151), `n` u16 @2, `class_id` i32 @4,
  `mapa` u32 @8 (referências dos 32 primeiros campos; extensão depois dos campos), `metadado`
  u32 @12 (RTI `id+1`). Relativo ao handle: estado `h-2`, flags `h-1`, classe `h+2`, mapa `h+6`,
  metadado `h+10`, campos `h+14`.
* **Páginas** de 64 KiB (1157), o primeiro 1 KiB é o mapa de marcas, 1 bit por palavra (1164,
  `bit_de_marca` 1168). Classes de tamanho por número de campos `0..=64` (`MAIOR_CLASSE`, 1202);
  objeto maior tem página própria arredondada a 64 KiB (`pagina_zerada`, 1828-1849; o bloco grande
  guarda `n` saturado em `u16::MAX` e a página sabe o real, `pagina_do_grande` 2094). Páginas do
  `ReservaDePaginas` (1556-1720; `mmap` em pedaços de 2 MiB só no Linux, `alloc_zeroed` nos demais).
* **Alocação:** `EspacoDeObjetos::alocar` (2116) tira de uma região *bump* por classe ou de faixas
  livres (`tirar_da_regiao`, 1887); a TLAB (`Contexto.tlab[n]`, `n ≤ TLAB_N = 16`, 1204; faixas
  de até 256 blocos, `reabastecer_tlab` 3738-3765, `devolver_tlabs` 3767-3786) serve a alocação
  em linha do emissor (`emitir_alocacao_em_linha`, `EN/llvm/mod.rs:2622-2676`: palavra
  `1 | n<<16 | cid<<32`).
* **Gatilhos:** menor a cada `LIMITE_JOVEM` = 2 MiB (1214) ou `CONTAGEM_JOVEM` = 256 Ki
  alocações (1221); completa quando `estimated_bytes` passa de `byte_threshold`
  (`recalcular_gatilhos`, 4859-4890, `CRESCIMENTO` = 150%, 1219); teto `DARTFORGE_HEAP_MAX_MB`.
* **Marcação** (`marcar::<MENOR>`, 4448-4562): pilha local; objeto do espaço promovido e marcado no
  bitmap, campos pelo mapa, primeiro filho seguido sem pilha; slot pelo `Value::trace`.
* **Varredura** menor (`varrer_jovens`, 2018-2091) e completa (`varrer`, 2162-2300) só leem os bits;
  folga de páginas vazias por classe (`demanda`, `pico`).
* **Barreira:** `get_mut` (4421-4437), `definir_campo` (3836), `campos_de_objeto` (4076),
  `dartforge_lembrar` (`RT/nucleo.rs:459`) e, no código gerado, `emitir_barreira`
  (`EN/llvm/mod.rs:2707-2728`: estado `== VELHO` → `dartforge_lembrar`).
* **Verificação:** `DARTFORGE_GC_VERIFICAR=1` confere cada coleta menor por uma travessia completa
  (4598-4638); `--gc-stress` é `DARTFORGE_GC_STRESS` (`RT/nucleo.rs:140`), sem TLAB, zerando mortos.
* **Corpo de fora** (`FORA`, `trocar_campos` 4020-4046): objeto cujo número de campos mudou (erro
  que ganha rastro, recarga J03) guarda os campos num corpo do `malloc`; `com_fora` solta os mortos.

### 1.7 O emissor

**Externs:** 275 em `EN/llvm/externs.rs:61-1236`, com efeitos `CONSERVADOR`/`ALOCA_SEM_LANCAR`/
`{aloca:false,lanca:false}` que alimentam `pode_coletar` (`EN/llvm/mod.rs:2810-2814`) e
`otimizar/efeitos.rs:21`. Por categoria: texto 43, `StringBuffer` 2, lista 30, mapa 10, conjunto
4, record 3, closure/ambiente/célula 21, caixas 6, tipados 12, SIMD 1, objeto 17, RTI 17,
exceções 43, GC/raízes 11, seletores 9, FFI 23, impressão 9, outros 14. **92 das 275** (texto,
SB, lista, mapa, conjunto, record) e **40** (closure/célula/caixas/tipados/SIMD) dependem da
representação. Nenhum código emite estas (só declaradas): `cell_get_tag`, `closure_code`,
`collection_is_unmodifiable`, `concurrent_modification_error_new`, `double_parse`, `env_get`,
`exception_peek_*`, `exception_take_*`, `format_exception_new`, `gc_collect`, `gc_empilhar`,
`gc_desempilhar`, `int_parse`, `int_try_parse`, `list_get_tag`, `list_remove_at`, `map_len`,
`marcar_permanente`, `null_assert_fail`, `object_class`, `print_list/_map/_null/_set/_string`,
`range_error_index/_new/_value`, `rti_classe_do_runtime`, `rti_classe_nome`, `rti_como`,
`rti_regra`, `rti_subtipo`, `set_len`, `stack_trace_from_string`, `string_equal`,
`string_juntar`, `string_len`, `object_campos`, `closure_tipada`.

**Helpers `df.*` em IR** (`EN/llvm/mod.rs`): `@df.obter_area` (3003-3022), `@df.preparar_area`
(233-235), `@df.classe` (3070-3104; com o SDK da fonte, `null`/`Smi` pelos imediatos de
`cids[0..2]`, 3049-3067), `@df.seletor` (3105-3118), `@df.subclasse` (3119-3149, `Contexto`
+352/+360/+368), `@df.caixa_int` (3151-3162), `@df.env_ref` (3163-3184), `@df.desencaixa_int`
(3185-3195), `@df.preparar_isolado` (1922-1976: `dartforge_registrar_cids` e as tabelas).

**`Contexto`** (`RT/heap.rs:2737-2792`, asserts 2830-2846): pendente 0, topo 8, áreas 16/24,
interrupção 32, `vazios` 40, registradas 48/56, `tlab[n]` 64+16n/72+16n, classes 336/344,
subtipos 352/360/368.

**Instruções da HIR que dependem da representação** (`EN/hir.rs`): `AllocList`, `AllocMap`,
`AllocRecord`, `AllocCell`/`CellGet`/`CellSet`, `AllocEnv`/`EnvGet`, `AllocClosure`/
`AllocClosureTipada`/`TearOff`, `Box`/`Unbox`, `JuntarTextos`, `Const(String/StringWtf8)`,
`CargaNativa`/`GravacaoNativa` sobre cabeçalhos. Onde nascem: `closures.rs` (AllocClosure 4,
AllocClosureTipada 2, AllocEnv 4, EnvGet 7, TearOff 3), `async_sm.rs` (AllocClosure 3,
AllocEnv 3, EnvGet 2), `locais.rs` (AllocCell 2, CellGet 2, CellSet 1, EnvGet 3), `tipados.rs`
(CargaNativa 8, GravacaoNativa 3), `textos.rs` (CargaNativa 2), `expressoes.rs` (AllocList,
AllocMap, AllocRecord, JuntarTextos), `literais.rs`, `fn_builder.rs` (Box 3, Unbox 3),
`sdk_fonte.rs`, `extensoes.rs`, `enums.rs`, `nsm.rs`, `padroes.rs`. A emissão está em
`EN/llvm/mod.rs`: `AllocList` 1047-1064, `AllocMap` 1065-1088, `AllocRecord` 1089-1108, caixas
1174-1204, `AllocCell`/`CellSet` 1472-1491, `EnvGet` 1495-1506, `JuntarTextos` 1507-1522,
`AllocEnv` 1523-1538, closures 1542-1560, literais de string com cache na área 578-605,
SIMD 1150-1169.

**A ABI de pares `(bits, tag)`** dos buffers da pilha usa tags 1 int, 2 bool, 3 ref, 4 double
(`tag_de`, `EN/llvm/mod.rs:1460-1467`; `RT/nucleo.rs:401-425`), diferentes das do `ValueTag` em
memória (0 int, 1 bool, 2 double, 3 ref).

**`cids_do_runtime`** (`EN/sdk_modulo.rs:144-183`): 32 posições (Null, `_Smi`, `_Mint`,
`_Double`, `bool`, `_OneByteString`, `_TwoByteString`, `_GrowableList`, `_List`,
`_ImmutableList`, `_Closure`, `_Record`, `_Uint8List`, `_Uint8ArrayView`, `_Int64List`,
`_Float32x4`, `_Int32x4`, `_Float64x2`, `_Int8List`…`_Float64x2List`, `_SendPort`,
`_Capability`), com o id que a numeração estável der (`context.rs:395-430`: ordem do caminho
estável a partir de 1, pulando 1000–1012). Chega ao runtime por `@df.cids` e
`dartforge_registrar_cids` (`RT/seletores.rs:227-238`), que refaz a classe de todo slot.
`ClassID.cidX` do SDK vira constante na compilação (`EN/lower/intrinsecos.rs:51-73`).

### 1.8 O SDK nativo

Sobreposições (`sdk_nativo/libraries.json:6-38`): `core/string_patch.dart` (`==` e buscas por
natives `DartForge_string_*`), `core/integers.dart` (`_Smi.toString` fora da tabela por
`DartForge_int_toString`, 650-661), `core/double.dart` (cache de `toString`), `core/array.dart`
(`DartForge_List_preencher`/`_copiar`), `core/growable_array.dart` (`_reservar`, cópias no
runtime), `core/string_buffer_patch.dart` (acumulador do runtime, `DartForge_sb_*`),
`collection/compact_hash.dart` (campos de verdade `_indiceDF`, `_hashMask`, `_data`,
`_usedData`, `_deletedKeys`, 161-178; natives `DartForge_hash_*`), `convert/convert_patch.dart`
(`_JsonListener._acrescentar`, parser com `chunk` num local), `core/function.dart` (`_Closure.hashCode`
sem cache), `core/regexp_patch.dart` (`_RegExp` com `_id` num vetor do runtime que nunca
encolhe, `RT/regexp.rs:1100-1102`), `core/finalizer_patch.dart`, `core/identical_patch.dart`,
`internal/print_patch.dart`, `internal/sort.dart`, `collection/list.dart`, async, isolate, ffi,
io, developer, mirrors. Sem sobreposição: `typed_data_patch.dart`, `internal_patch.dart`
(`allocateOneByteString` 43, `writeIntoOneByteString` 49), `record_patch.dart`,
`function_patch.dart`, `class_id_fasta.dart`.

* `_List`, `_GrowableList` e `_ImmutableList` **não** têm campos em Dart: são o mesmo
  `Value::List`, com a classe pelas marcas do slot (`RT/seletores.rs:272-280`). Natives de lista
  em `RT/nativos_listas.rs`: `List_allocate` 203, `List_getLength` 219, `List_setIndexed` 225,
  `List_slice` 257, `ImmutableList_from` 274, `GrowableList_allocate` 293, `_getCapacity` 310,
  `_getLength` 323, `_setLength` 329, `_setData` 343, `_setIndexed` 473,
  `DartForge_List_preencher` 368, `_copiar` 428, `DartForge_GrowableList_reservar` 453,
  `DartForge_json_acrescentar` 148, `Internal_makeListFixedLength` 625,
  `Internal_makeFixedListUnmodifiable` 643, `DartForge_lista_get` 245.
* `_Map`/`_Set`/`_ConstMap`/`_ConstSet` **já** são objetos Dart do espaço (literais por
  `colecao_vazia_fonte`, `EN/lower/sdk_fonte.rs:1924-1935`); o `_index` é um `Value::TypedData`
  (`Uint32List`) e o `_data` um `Value::List`. `nativos_hash.rs` sonda a mesma tabela
  (`sondar` 122-159) para chaves `int` e `String`.
* Listas tipadas: `Value::TypedData`/`TypedView`; fábricas reconhecidas em `fabrica_tipada`
  (`EN/lower/sdk_fonte.rs:801-916`) → `dartforge_typed_novo`/`dartforge_view_nova`;
  `asTypedList` → `dartforge_typed_externo` (`RT/ffi.rs:778`), classe `_XList` interna com
  `Armazenamento::externo`.
* `_Closure` tem campos declarados (`_function`, `_context`…) que o runtime não usa;
  `Closure_equals` (`RT/nativos_listas.rs:829-847`) e `Closure_computeHash` (1060-1074) olham o
  `Value::Closure`/`Environment`. `Function_apply`: `RT/closures.rs:269-306`.
* Record posicional: `Value::Record`, classe `_Record`; nomeado: objeto do espaço com id
  `ID_BASE_DE_FORMA + k` (`EN/context.rs:94`, `EN/lower/registros.rs:1-12`), campos todos `Ref`.
* Catálogo `EN/nativos.rs`: 534 natives (495 `Runtime`, 38 `Pendente`, 1 `Embutido`), 90
  intrínsecos (`INTRINSECOS`, 81-172), `RETORNO_REF` só `Double_parse` (77).
* O caminho sem SDK da fonte (`DARTFORGE_SDK_DA_FONTE=0`, `EN/sdk_modulo.rs:122-128`) ainda
  existe: `lower/sdk_por_nome.rs` (congelado; com o SDK ligado só encaminha ao despacho por
  seletor) e os externs `dartforge_string_*`/`list_*`/`map_*`/`set_*` de `RT/strings.rs` e
  `RT/colecoes.rs`. É usado por `EN/tests/contrato.rs:36` e `crates/jit/tests/sessao_persistente.rs:148`.

### 1.9 Isolados, recarga, RTI e E/S

* **Mensagens** (`RT/portas.rs`): `NoG` (44-71) espelha variante por variante o `Value`;
  `copiar_para_grafo` (159-276) casa `heap.get(h)`; `materializar` (482-686) aloca um nó vazio da
  mesma variante e depois o preenche por `get_mut` (a hipótese "o slot nunca troca de variante",
  `RT/heap.rs:2801-2802`). Mesmo isolado: permanentes e **qualquer string** passam por identidade
  (`ref_de_handle`, 100-134). Entre isolados: constante canônica vai pelo getter, tear-off pelo
  código. `Isolate.exit` sempre copia (`RT/isolados.rs:562-573`).
* **Recarga (J03):** `migrar_instancias` (`RT/heap.rs:3989-4015`) percorre só o espaço de
  objetos (`para_cada_vivo`) e troca campos por `trocar_campos` (corpo de fora); o plano vem de
  `crates/jit/src/migracao.rs:67-142` e é aplicado por isolado em `aplicar_migracao_pendente`
  (`RT/seletores.rs:586-601`). `dartforge_publicar_geracao` (`RT/seletores.rs:182-212`) refaz
  tabelas e esvazia caches. A memória de uma geração do JIT **é liberada** quando nenhuma entrada
  a usa (`docs/JIT.md` §J02) — dado estático de um módulo do JIT não pode ser referenciado pelo heap.
* **RTI** (`RT/tipos.rs`): `chave_do_valor` (133-167) lê o metadado direto do cabeçalho do objeto
  (146) e, para slot, casa a variante (`Value::String` → −4, 162); `tipo_do_valor` (781-841) casa
  todas as variantes; a lista guarda `E` no metadado do slot e a forma compacta sai dele
  (`forma_da_lista_do_tipo` 901, `ajustar_forma_da_lista` 928). O cache `P<i>` do emissor lê
  `metadado`/`class_id` em `h+10`/`h+2` (`EN/llvm/mod.rs:1816-1850`).
* **E/S:** nenhuma operação assíncrona guarda ponteiro para memória do heap: IOCP copia para
  `Operacao.dados` (`RT/io_windows_eventos.rs:292-348,1037-1077`), soquetes e arquivos copiam
  (`bytes_da_lista_tipada`, `RT/io_arquivos.rs:224-232`). Ponteiros para dentro do heap só em
  chamadas síncronas: `com_memoria` (`RT/ffi.rs:262-282`), `dartforge_ffi_endereco_do_composto`
  (814-829), `dartforge_typed_ptr` (`RT/typed_data.rs:255-264`), `tls.rs:656-693`.

### 1.10 O que a VM do Dart e a Julia fazem (base das decisões)

**VM do Dart** (`VM/runtime/vm/`):

* Cabeçalho de uma palavra (`raw_object.h:192-329`): bit 0 `CardRemembered`, bits de barreira
  `NotMarked`/`NewOrEvacuationCandidate`/`AlwaysSet`/`OldAndNotRemembered` (206-225, sobrepostos
  por `kBarrierOverlapShift = 2`, 233), size tag (271-273), cid de 20 bits na posição 12
  (312-316), hash de 32 bits nos bits altos em 64 bits (`HASH_IN_OBJECT_HEADER`, 318-329;
  `globals.h:145-147`).
* Layouts (offsets x64 sem compressão, `compiler/runtime_offsets_extracted.h` 14697-15647):
  `OneByteString`/`TwoByteString` = cabeçalho + `length` (Smi, +8) + unidades (+16), hash no
  cabeçalho (`raw_object.h:3300-3346`); `Array`/`ImmutableArray` = `type_arguments` (+8),
  `length` Smi (+16), elementos (+24) (3520-3559); `GrowableObjectArray` = `type_arguments`,
  `length`, `data` (3561-3572); `Mint`/`Double` = cabeçalho + valor, 16 B (3273-3298);
  `TypedData` com o ponteiro interno `data_` em +8 comum a interna, externa e visão
  (`PointerBase`, 3353-3380), `length` Smi e payload em +24 (3411-3447); `TypedDataView` =
  `typed_data` + `offset_in_bytes` (3450-3509); `LinkedHashBase` = `type_arguments`, `hash_mask`,
  `data`, `used_data`, `deleted_keys`, `index` (3574-3613); `Record` = `shape` Smi + campos
  (3668-3686); `Context` = `num_variables`, `parent`, variáveis (2631-2646); `Closure` com
  payload variável (3142-3259).
* Cids predefinidos em ordem fixa (`class_id.h:302-334`), com predicados por faixa
  (`IsStringClassId` 452-454, `IsArrayClassId` 460-464, grupos de 4 por tipo de lista tipada
  296-300 e 348-352).
* `String::Hash` (`object.cc:24118-24177`, `hash.h:13-30`): o `StringHasher` soma unidade a
  unidade, finaliza em 30 bits (`kHashBits`, `object.h:357`) e troca 0 por 1; guardado no
  cabeçalho na primeira consulta (`SetCachedHashIfNotSet`).
* Barreira (`assembler_x64.cc:1676-1781`, stub `stub_code_compiler_x64.cc:2042-2182`): pula `Smi`,
  testa `(tags_do_objeto >> 2) & tags_do_valor & máscara` — velho e não lembrado recebendo valor
  novo; o stub lembra o objeto (store buffer, blocos de 1024) ou marca o **cartão** se o objeto é
  `CardRemembered`.
* Cartões (`heap/page.h:186-189,286-307`): 32 slots por cartão, 1 bit por cartão, só em páginas
  grandes; `Array::UseCardMarkingForAllocation` quando a instância passa de 256 KiB
  (`object.h:11163-11166`); a marcação e o scavenge percorrem só os cartões sujos
  (`marker.cc:206-248`, `page.cc:198-261`).
* Tamanhos: página de 512 KiB (`page.h:52`), objetos ≥ 64 KiB em página grande
  (`spaces.h:54-58`, `pages.cc:355-381`), new space até 256 KiB por objeto (`heap.h:67`).
* Alocação em linha: `TryAllocateObject`/`TryAllocateArray` (`assembler_x64.cc:2550-2623`);
  `TryAllocateString` (`asm_intrinsifier_x64.cc:1597-1699`); `writeIntoOneByteString` é um
  `movb` (1754-1764).
* Intrínsecos: `[]` de `Array`/`GrowableList` carrega `length`, confere, carrega `data` e
  indexa (`kernel_to_il.cc:1191-1235`); `codeUnitAt` confere o comprimento, carrega o cid e lê 1
  ou 2 bytes (1394-1438); `_setIndexed` com barreira de array, `_setLength` sem barreira
  (`graph_intrinsifier.cc:523-626`); `LoadClassId` = `movl` + `shrl 12`
  (`assembler_x64.cc:2823-2828`).
* Cópia entre isolados (`object_graph_copy.cc:165-201`): compartilha o canônico e o
  profundamente imutável; copia `ExternalTypedData` para memória nova (331-343).
* Recarga: `InstanceMorpher::CreateMorphedCopies` (`isolate_reload.cc:243-373`) aloca a cópia com
  o layout novo e faz `become` (`heap/become.cc:290-340`).
* `_GrowableList._nextCapacity(old) = (old * 2) | 3` (`sdk/lib/_internal/vm/lib/growable_array.dart:385`).

**Julia** (`JL/`):

* Pools por classe de tamanho até 2032 B (`julia_internal.h:546-589,679`), página de 16 KiB;
  alocação por lista livre ou *bump* na página nova (`gc-stock.c:828-883`); objetos maiores são
  `bigval_t` do `malloc` numa lista duplamente ligada (`gc-stock.c:517-554`, `gc-common.h:34-54`).
* Memória de arrays grandes: cabeçalho no pool e dados no `malloc` rastreado em `mallocarrays`,
  soltos na varredura (`genericmemory.c:30-54`, `gc-stock.c:755-783`); contam para o gatilho da
  coleta (`jl_gc_count_allocd`, 669-675).
* Barreira de um teste: pai `GC_OLD_MARKED` e filho não marcado → `jl_gc_queue_root`, que tira o
  bit `GC_OLD` para não redisparar (`gc-wb-stock.h:19-24`, `gc-stock.c:1650-1681`). Sem cartões:
  arrays grandes vão inteiros ao remset e a marcação os percorre em pedaços de 65 536
  (`gc-stock.c:1996-2060`).
* No LLVM, a barreira é teste de bits em linha e chamada lenta com peso 1:9
  (`llvm-final-gc-lowering-stock.cpp:57-104`); a alocação é chamada ao C com o offset do pool
  calculado ao compilar (`8-55`).
* Varredura rápida pula a página sem jovens (`gc-stock.c:1115-1124`).

---

## 2. O desenho final

### 2.1 Princípios

1. **Um espaço, um coletor.** Todo valor do runtime é um bloco do espaço de objetos. O coletor
   continua geracional e **sem mover** (marcas pegajosas, varredura pelo mapa de marcas), porque o
   código gerado guarda handles em registradores entre pontos de coleta e a pilha-sombra só
   enraíza (NATIVO-PLANO §8.6). Não mover também dá ponteiros estáveis a FFI, TLS e E/S.
2. **O coletor não precisa da classe.** O formato do corpo está no cabeçalho (`flags`); a classe
   só serve ao programa, ao despacho e ao runtime.
3. **Classes do runtime fixas.** O cid de cada classe que o runtime cria é uma constante do
   contrato (§2.4), igual no runtime, no emissor, no JIT e em todas as gerações.
4. **O cid não muda depois que o objeto é publicado.** `df.classe` lê o cid com
   `!invariant.load`; mudar de classe é alocar outro objeto (§2.16).
5. **Uma palavra por elemento.** Elementos e campos têm 8 bytes; a `TaggedValue` de 16 bytes sai do
   heap. Onde a palavra precisa se descrever sozinha (listas gerais, records), ela é um `Ref`
   (`0`, `Smi` ímpar, handle); o escalar sem caixa só vive onde a forma diz o tipo (listas
   compactas, campos com o bit do mapa, listas tipadas).

### 2.2 O handle

| `i64` | Significado |
|---|---|
| `0` | null (classe `Null`, cid 1) |
| ímpar | `Smi` (cid 2), `(v << 1) \| 1` — inalterado |
| `h & 7 == 2`, `h > 0` | objeto: bloco em `h - 2` (blocos alinhados a 8) |
| qualquer outro | inválido (N4) |

Somem os handles múltiplos de 4. `e_objeto(h)` continua `h & (3 | i64::MIN) == 2`. Os objetos
estáticos (§2.11) usam a mesma codificação. O handle continua um `i64` em toda a ABI.

### 2.3 O cabeçalho

Os mesmos 16 bytes de hoje (`RT/heap.rs:1108-1134`); o que muda é o uso de `flags`, um estado novo
e o `mapa` das strings.

| Byte | Campo | Conteúdo |
|---|---|---|
| 0 | `estado: u8` | `LIVRE` 0, `JOVEM` 1, (2 reservado), `VELHO` 3, `LEMBRADO` 4, **`PERMANENTE` 5** (objeto estático fora do heap: nunca marcado, varrido, lembrado nem gravado) |
| 1 | `flags: u8` | bit 0 `FORA` (0x01, inalterado); bits 1-2 **`FORMA`**: `INSTANCIA` 0x00, `BRUTO` 0x02, `REFS` 0x04 (0x06 reservado); bit 3 **`CARTOES`** 0x08; bit 4 **`ANEXO`** 0x10; bits 5-6 **`ELEMENTO`** da lista compacta: `INT` 0x20, `DOUBLE` 0x40, `BOOL` 0x60; bit 7 **`EXTERNO`** 0x80 (lista tipada sobre memória de fora) |
| 2-3 | `n: u16` | `INSTANCIA`: número de campos; `BRUTO`/`REFS`: palavras do corpo; saturado em `u16::MAX` no objeto grande (a região sabe o real) |
| 4-7 | `class_id: i32` | o cid |
| 8-11 | `mapa: u32` | `INSTANCIA`: bits de referência dos 32 primeiros campos (inalterado); **strings: o hash** (30 bits, 0 = ainda não calculado); demais: 0 |
| 12-15 | `metadado: u32` | RTI `id + 1` (0 = nenhum), para todo objeto que tem argumentos de tipo |

A alocação em linha grava a palavra 0 inteira: `estado | flags << 8 | n << 16 | cid << 32`; a
palavra 1 vem zerada do bloco livre.

**Regra:** nada lê `mapa` como mapa de referências sem antes conferir `FORMA == INSTANCIA`
(coletor, verificação, cópia entre isolados, migração).

Formatos do corpo (as palavras depois do cabeçalho):

| `FORMA` | Palavras | O coletor | Tamanho do corpo |
|---|---|---|---|
| `INSTANCIA` | `n` campos (bits crus ou `Ref`) + extensão do mapa (`palavras_do_mapa(n)`) | segue os campos com bit aceso (hoje) | `capacidade(n) + palavras_do_mapa(n)` |
| `BRUTO` | bytes sem referência | nada | `n` |
| `REFS` | palavra 0: comprimento ou forma, **bruto**; palavras 1..: `Ref` | segue toda palavra par e não nula das palavras 1.. (em objeto `LEMBRADO` com `CARTOES`, só as dos cartões sujos) | `n` |

### 2.4 Cids fixos

A numeração estável de `EN/context.rs:395-430` passa a reservar **1–127** para as classes abaixo
(quem não existe no SDK carregado fica sem instâncias) e começa as demais em **128**. A faixa
1000–1012, `ID_QUADRO_ASYNC` (0x3FFF_FF00, `EN/lower/async_sm.rs:53`), `CLASSE_TIPO`
(0x3FFF_FF01) e as formas de record (`ID_BASE_DE_FORMA` = 0x4000_0000, `EN/context.rs:94`)
ficam como estão.

| cid | Classe | Biblioteca | Formato |
|---:|---|---|---|
| 0 | (bloco livre) | — | — |
| 1 | `Null` | core | (só o `0`) |
| 2 | `_Smi` | core | (imediato) |
| 3 | `_Mint` | core | `BRUTO` |
| 4 | `_Double` | core | `BRUTO` |
| 5 | `bool` | core | `BRUTO`, só as duas estáticas |
| 6 | `_OneByteString` | core | `BRUTO` |
| 7 | `_TwoByteString` | core | `BRUTO` |
| 8 | `_List` | core | `REFS` ou `BRUTO`+`ELEMENTO` |
| 9 | `_ImmutableList` | core | `REFS` ou `BRUTO`+`ELEMENTO` |
| 10 | `_GrowableList` | core | `INSTANCIA` |
| 11 | `_Closure` | core | `INSTANCIA` |
| 12 | `_Record` | core | `REFS` |
| 13 | `_Contexto` | interna do runtime | `INSTANCIA` |
| 14 | `_Celula` | interna | `INSTANCIA` |
| 15 | `_AcumuladorDeTexto` | interna (o `StringBuffer`) | `BRUTO`+`ANEXO` |
| 16 | `_ProgramaDeRegExp` | interna | `BRUTO`+`ANEXO` |
| 17 | `_SendPort` | isolate | `INSTANCIA` (1 campo, o id; como hoje) |
| 18 | `_Capability` | isolate | idem |
| 19–21 | `_Float32x4`, `_Int32x4`, `_Float64x2` | typed_data | `BRUTO` |
| 22 + t | `_Int8List` … `_Float64x2List` (t = `TIPO_*` 0–13 de `RT/typed_data.rs:13-26`) | typed_data | `BRUTO` |
| 36 + t | `_Int8ArrayView` … `_Float64x2ArrayView` | typed_data | `INSTANCIA` |
| 50 + t | `_UnmodifiableInt8ArrayView` … | typed_data | `INSTANCIA` |
| 64 | `_ByteDataView` | typed_data | `INSTANCIA` |
| 65 | `_UnmodifiableByteDataView` | typed_data | `INSTANCIA` |
| 66–127 | reservados | | |

Predicados por faixa (como `class_id.h`): string `cid − 6 <u 2`; `_List`/`_ImmutableList`
`cid − 8 <u 2`; lista do núcleo `cid − 8 <u 3`; lista tipada interna `cid − 22 <u 14`; qualquer
lista tipada ou visão `cid − 22 <u 44`; visão não modificável `cid − 50 <u 14 || cid == 65`.

Consequências: `@df.classe` não chama o runtime (§3.5); `ClassID.cidX` vira a constante do
contrato (sem busca em `EN/lower/intrinsecos.rs:51-73`); `@df.cids` e `dartforge_registrar_cids`
viram uma conferência de ABI (o runtime aborta se a tabela do módulo não é a dele — o marcador de
ABI do `PLANO-TAMANHO-DESEMPENHO.md` §4.2); o problema "string alocada antes de os cids chegarem"
não existe. O `_ExternalXArray` da VM não é usado: `asTypedList` devolve a `_XList` interna com
`EXTERNO`, como hoje (`RT/ffi.rs:778-800`).

### 2.5 Layout de cada classe

Deslocamentos a partir do bloco `b`; o handle é `b + 2`, então a palavra em `b + k` está em
`h + k − 2`. "Bruto" = bits sem etiqueta; "`Ref`" = `0`, `Smi` ou handle.

| Classe (cid) | Formato | `n` | `b+16` | `b+24` | `b+32` | `b+40…` | `mapa` | `metadado` |
|---|---|---|---|---|---|---|---|---|
| `_OneByteString` (6) | `BRUTO` | `1 + ⌈len/8⌉` | comprimento (bruto, unidades) | unidades `u8`… (zeros até a palavra) | | | hash | 0 |
| `_TwoByteString` (7) | `BRUTO` | `1 + ⌈2·len/8⌉` | comprimento | unidades `u16`… | | | hash | 0 |
| `_Mint` (3) | `BRUTO` | 1 | valor `i64` | | | | 0 | 0 |
| `_Double` (4) | `BRUTO` | 1 | valor `f64` | | | | 0 | 0 |
| `bool` (5) | `BRUTO`, `PERMANENTE` | 1 | 0 ou 1 | | | | 0 | 0 |
| `_List`/`_ImmutableList` geral (8/9) | `REFS` | `1 + len` (+ cartões) | comprimento (bruto) | elemento 0 (`Ref`) | elemento 1 | … | 0 | RTI `_List<E>` |
| `_List`/`_ImmutableList` compacta | `BRUTO`+`ELEMENTO` | `1 + len` | comprimento | elemento 0: `i64` (`INT`), bits `f64` (`DOUBLE`) ou 0/1 (`BOOL`) | … | | 0 | RTI |
| `_GrowableList` (10) | `INSTANCIA` | 2 | comprimento (bruto) | dados (`Ref` → `_List`; bit 1) | | | `0b10` | RTI `_GrowableList<E>` |
| `_Closure` (11) | `INSTANCIA` | 4 | código (bruto, o `code_id`) | contexto (`Ref`, ou o valor capturado no "ambiente direto", `EN/lower/closures.rs:224-244`; bit 1 por gravação) | corpo tipado (bruto, endereço ou 0) | ABI (bruto) | por gravação | RTI da função, se houver |
| `_Record` (12) | `REFS` | `1 + k` | forma: número de campos (bruto) | campo 0 (`Ref`) | campo 1 | … | 0 | 0 |
| `_Contexto` (13) | `INSTANCIA` | k ≥ 1 | captura 0 (bruta ou `Ref`, bit por gravação) | captura 1 | … | | por gravação | 0 |
| `_Celula` (14) | `INSTANCIA` | 1 | o valor (bit por gravação) | | | | por gravação | 0 |
| `_AcumuladorDeTexto` (15) | `BRUTO`+`ANEXO` | 1 | `*mut Vec<u16>` | | | | 0 | 0 |
| `_ProgramaDeRegExp` (16) | `BRUTO`+`ANEXO` | 1 | `*mut ProgramaRe` | | | | 0 | 0 |
| `_SendPort`/`_Capability` (17/18) | `INSTANCIA` | 1 | id (bruto) | | | | 0 | 0 |
| SIMD (19–21) | `BRUTO` | 2 | pistas, 16 bytes | | | | 0 | 0 |
| lista tipada interna (22–35) | `BRUTO` | `2 + ⌈len·s/8⌉` | comprimento (elementos) | dados (bruto: `b+32`, ou a memória de fora com `EXTERNO`) | bytes… | | 0 | 0 |
| lista tipada externa | `BRUTO`+`EXTERNO` | 2 | comprimento | endereço nativo | | | 0 | 0 |
| visão (36–65) | `INSTANCIA` | 4 | comprimento (elementos) | dados (bruto: `base.dados + deslocamento`) | base (`Ref`, bit 2) | deslocamento em bytes | `0b100` | 0 |

Observações:

* **Comprimento e dados no mesmo lugar em toda lista tipada**, interna, externa ou visão (o
  `PointerBase::data_` da VM, `raw_object.h:3353-3380`): o código gerado lê `len = [h+14]` e
  `p = [h+22]` sem saber qual das três é. O `dados` de uma visão é calculado na criação: a base não
  se move.
* O `_Record` com campo nomeado continua sendo objeto da classe da forma (`INSTANCIA`, campos
  todos `Ref`, `EN/lower/registros.rs:1-12`); só o posicional muda de `Value::Record` para cid 12.
* O `_Contexto` mantém o modelo de hoje (capturas imutáveis mais células para as mutáveis); o
  modelo da VM (variáveis no contexto, cadeia de pais) fica como trabalho futuro.
* `_Map`, `_Set`, `_ConstMap`, `_ConstSet` não mudam: são objetos Dart com os campos da sobreposição
  (`sdk_nativo/collection/compact_hash.dart:161-178`); o `_data` passa a ser `_List` geral (cid 8,
  `REFS`) e o `_index` uma `_Uint32List` (cid 28).
* `_ByteBuffer` é classe Dart comum (campo `_data`), sem cid fixo.

### 2.6 Alocação

**Classes de tamanho** por palavras do corpo `w` (bloco = `16 + 8w` bytes):

* exatas `1..=64` (a de hoje; 0 campos usa 1 palavra, `capacidade(0) = 1`);
* **24 médias**, escolhidas para encher a página (k blocos em 64 512 bytes úteis):
  `70, 82, 98, 110, 124, 142, 166, 199, 222, 250, 286, 334, 401, 446, 502, 574, 670, 804, 894,
  1006, 1150, 1342, 1610, 2014` (112, 96, 80, 72, 64, 56, 48, 40, 36, 32, 28, 24, 20, 18, 16,
  14, 12, 10, 9, 8, 7, 6, 5, 4 blocos por página). A perda interna máxima é de 20% (25% entre
  1610 e 2014);
* **grande**: `w > 2014` (corpo acima de ~16 KiB) numa região própria, `1 KiB (mapa de marcas) +
  bloco + cartões`, arredondada a **4 KiB** e alinhada a 64 KiB: `VirtualAlloc(MEM_RESERVE |
  MEM_COMMIT)` no Windows (a granularidade de alocação já é 64 KiB), `mmap` com o recorte de
  `mapeamento::mapear` (`RT/heap.rs:1587-1611`) no Linux e no macOS. Hoje a página grande é um
  `alloc_zeroed` arredondado a 64 KiB (`RT/heap.rs:1841,1852-1863`): um objeto de 20 KiB ocuparia
  64 KiB.

`EspacoDeObjetos` passa a indexar os vetores por classe (`N_CLASSES = 89`: 1–64 exatas mais 24
médias; índice 0 livre), não por número de campos: `livres`, `regiao`, `demanda`, `pico`,
`em_uso`, `regiao_suja`. `Pagina` guarda as palavras do bloco. A página vazia reusada pode ser
formatada para qualquer classe exata ou média (`pagina_zerada`, 1828). Um objeto `INSTANCIA` de
`n` campos pede `capacidade(n) + palavras_do_mapa(n)` palavras.

Referência: as classes de tamanho da Julia (`julia_internal.h:546-589`) são calculadas do mesmo
modo (máximo empacotamento por página); a VM usa listas livres de tamanho variável no *old space*
(`heap/freelist.h`), que exigiriam varredura por objeto.

**TLAB:** a de hoje, indexada por palavras `1..=16` (`Contexto.tlab[w]` em `64 + 16w`). O código
gerado aloca em linha qualquer objeto de até 16 palavras, com `w` constante ou calculado (strings,
listas pequenas, caixas, células, contextos, closures, visões): `@df.alocar` (§3.5). Acima de 16
palavras, ou TLAB esgotada, `dartforge_alocar` (§3.6), que coleta se preciso e reabastece.

**No runtime:** `Heap::alocar(cid, w, flags)` (§3.2); todo objeto sai zerado. A contagem para os
gatilhos continua em `estimated_bytes`, agora `Σ tamanho dos blocos em uso + bytes externos`
(anexos, corpos de fora, regiões grandes pelo tamanho real), como o `heap_size` da Julia
(`gc-stock.c:498-507,669-675`).

### 2.7 Barreira de escrita e cartões

**Barreira de objeto** (depois de gravar o `Ref` `v` num campo ou elemento do objeto `o`):

```
se o.estado == VELHO e v é objeto e v.estado == JOVEM: dartforge_lembrar(o)
```

É a de hoje (`EN/llvm/mod.rs:2707-2728`) mais o teste do filho, como a VM (`StoreBarrier`, o filho
precisa estar no *new space*) e a Julia (`jl_gc_wb`, filho não marcado): gravar num velho um
objeto velho, estático ou um `Smi` não lembra nada. `LEMBRADO` (4) não redispara. A função lenta
continua `dartforge_lembrar` (`VELHO → LEMBRADO`, push em `lembrados`).

**Cartões** (`CARTOES`): toda `_List`/`_ImmutableList` geral com mais de 2014 palavras (sempre
objeto grande) tem, depois do último elemento, `⌈len / 2048⌉` palavras de cartões — 1 bit por 32
elementos, o `kSlotsPerCardLog2 = 5` da VM (`page.h:186-189`). A barreira de elemento:

```
se o.estado ≥ VELHO (3 ou 4) e v é objeto jovem:
    se o.flags & CARTOES: cartões[i >> 11] |= 1 << ((i >> 5) & 63)     // em b + 24 + 8·len
    se o.estado == VELHO: dartforge_lembrar(o)
```

Na coleta menor, um lembrado com `CARTOES` tem só os elementos dos cartões sujos percorridos, e
os cartões são zerados; sem `CARTOES`, o objeto inteiro (hoje). A completa percorre tudo e zera os
cartões. Sem cartões, uma lista velha de um milhão de elementos com uma gravação por ciclo custaria
8 MiB lidos por coleta menor (a Julia aceita isso e fatia a marcação; a VM usa cartões).

**Gravações do runtime:** passam todas por `Heap::gravar_ref`/`gravar_refs`/`definir_campo` (§3.2)
e `gravar_celula` (§3.3), que aplicam a mesma regra. Gravar escalar não tem barreira. `metadado`, `hash` e `flags` de forma não são
referências e não têm barreira.

### 2.8 Coletor

* **Marcação** (`marcar::<MENOR>`, hoje `RT/heap.rs:4448-4562`): para cada handle par e não nulo,
  lê o cabeçalho; `PERMANENTE` é ignorado **antes** de qualquer acesso ao mapa de marcas (o mapa de
  um estático cairia fora de uma página do heap); `percorre` = `JOVEM` na menor, `!marcado` na
  completa. Depois de marcar, despacha por `FORMA`: `INSTANCIA` pelo mapa (hoje); `BRUTO` nada;
  `REFS` percorre as palavras `1..w` (w do cabeçalho ou, no objeto grande, da região), empilhando
  as pares e não nulas. O "primeiro filho sem pilha" continua valendo para `INSTANCIA` e `REFS`.
* **Lembrados** na menor: `INSTANCIA` pelo mapa, `REFS` inteiro ou pelos cartões (§2.7).
* **Varredura:** a de hoje (só bits), por classe; a região grande sem bit volta ao sistema
  (`VirtualFree(MEM_RELEASE)`/`munmap`). Os **anexos** (`ANEXO`): lista `anexos_nativos` de
  `(bloco, soltar: unsafe fn(*mut u8), ptr, bytes)` percorrida como `com_fora`
  (`soltar_corpos_mortos`, `RT/heap.rs:1990-2005`): sem bit, chama `soltar(ptr)` e desconta
  `bytes` de `estimated_bytes`.
* **Tabelas laterais** por handle continuam (fracas, efêmeros, finalizáveis, anexos de
  finalizador, `late`) e são purgadas pelo bit de marca, com `PERMANENTE` sempre vivo. Somem as
  dos slots e `imutaveis_do_espaco` (a imutabilidade é classe, §2.16); `iteracoes_ativas` e
  `origens` saem com o legado de `colecoes.rs`.
* **`permanentes` e `constantes`** (hoje nem raízes nem purgadas, §1.5): passam a ser purgadas pelo
  bit de marca como as demais; quem precisa que a constante viva a mantém por um global (o getter
  já grava no global, `EN/lower/constantes.rs:486-505`).
* **Verificação** (`DARTFORGE_GC_VERIFICAR`): a travessia completa passa a respeitar `FORMA` e a
  conferir que todo elemento jovem de uma `REFS` lembrada com `CARTOES` está num cartão sujo.
* **Gatilhos:** os de hoje. Como strings, listas e caixas passam a contar blocos (e não
  `size_of::<Value>() + capacidade`), `LIMITE_JOVEM` e `CONTAGEM_JOVEM` devem ser remedidos (P0),
  sem mudar o critério.

### 2.9 Raízes

Inalteradas: pilha-sombra (`QuadroDeRaizes`, `RT/heap.rs:2724-2729`), `globais` (chave = endereço
do slot da área, `RT/gc_raizes.rs:56`, `EN/llvm/mod.rs:1223-1233`), `raizes_do_runtime`, `frames`
do runtime (`com_raizes`, `RT/gc_raizes.rs:8-20`), `enum_values`, `tearoffs`, `literais`,
`finalizacoes_prontas`, anexos de finalizador. Sai `caixas_bool` (as duas são estáticas). Os
estáticos não são raízes: não morrem e não apontam para o heap.

### 2.10 Identidade e hash

* `identical(a, b)`: mesmo handle; ou os dois `_Mint` (cid 3) com o mesmo valor; ou os dois
  `_Double` (cid 4) com os mesmos bits (`Instance::IsIdenticalTo` da VM; R9). Em linha
  (`@df.identico`, §3.5). A forma canônica garante que um `_Mint` nunca está na faixa do `Smi`.
* `identityHashCode`/`Object.hashCode` padrão: do endereço (`hash_de_identidade`,
  `RT/heap.rs:3803-3813`), que não muda porque o coletor não move — a VM guarda um número
  sorteado no cabeçalho (`object.cc:20997-21040`), que aqui não é preciso. `String`: o hash do
  conteúdo (como a VM, `_identityHashCode` = `String_getHashCode`); `int`: o valor; `double`,
  `null`, `true`, `false`: as regras e constantes da VM (2011, 1231, 1237).
* `String.hashCode`: o `StringHasher` da VM (`Texto::hash_vm`, `RT/heap.rs:388-400`, igual a
  `hash.h:13-30`), guardado em `mapa` na primeira consulta; nos literais estáticos, calculado pelo
  emissor com a mesma função (`layout::hash_de_texto`). Somem `hashes_de_texto` e
  `Heap::hash_de_texto` pelo slot.

### 2.11 Literais e constantes

**Literal de string no AOT = objeto estático.** Cada `Const(String/StringWtf8)` vira uma
constante do módulo, na forma do bloco:

```llvm
@df.s.<chave> = linkonce_odr constant { i64, i64, i64, [<N> x i8] }
    { i64 <PERMANENTE | BRUTO<<8 | w<<16 | cid<<32>, i64 <hash>, i64 <len>, [<N> x i8] <unidades> },
    comdat, section "<seção da imagem>", align 8
```

e o valor é a expressão constante `ptrtoint (ptr getelementptr (i8, ptr @df.s.<chave>, i64 2) to
i64)`: sem chamada, sem cache no ponto de uso (`EN/llvm/mod.rs:578-605` sai), sem
`LITERAIS_POR_ENDERECO`. `<chave>` é o blake3 do cid e das unidades; o `comdat` com
`linkonce_odr` faz o ligador manter uma cópia só entre o programa e os módulos do SDK em cache,
então `identical('a', 'a')` vale entre bibliotecas (a canonicalização de constantes da
especificação). A seção é própria (`.dfimg$m` no COFF, `dfimg` no ELF, `__DATA_CONST,__dfimg` no
Mach-O), só leitura: o runtime define marcadores de início e fim (`.dfimg$a`/`.dfimg$z` no COFF,
`__start_dfimg`/`__stop_dfimg` no ELF, `section$start$__DATA_CONST$__dfimg` no Mach-O, mais um
objeto seu para a seção sempre existir) e reconhece um estático pela faixa quando valida um handle
(`--gc-stress`, verificação); fora disso basta `estado == PERMANENTE`. Os estáticos são
compartilhados por todos os isolados — a "região permanente, somente leitura, de constantes
canônicas" da decisão 4 do proprietário (NATIVO-PLANO §7.1).

As caixas de `bool` são estáticas **do runtime** (`dartforge_falso`, `dartforge_verdadeiro`,
mesma seção): `Box` de `bool` é um `select` entre duas constantes.

**No JIT não há estáticos.** A memória de uma geração é liberada (J02, `docs/JIT.md` §"Política de
retenção"); um literal apontado pelo heap não pode morar nela. O JIT continua com
`dartforge_string_new` e o cache no ponto de uso, internando no heap (`Heap::literais`, raiz).
O emissor decide por uma opção do módulo (`objetos_estaticos`, verdadeira no AOT e na DLL do SDK
de produção).

**Constantes que não são string** (`const [..]`, `const {..}`, instâncias `const`) continuam
criadas pelo getter canônico em tempo de execução e guardadas num global
(`EN/lower/constantes.rs:381-505`), agora como objetos do espaço. Torná-las estáticas exige RTI
sem id por isolado no metadado (os ids de tipo são por isolado, `RT/tipos.rs:201-232`); fica fora
desta especificação.

### 2.12 Isolados

O grafo da mensagem (`RT/portas.rs:44-88`) deixa de espelhar o `Value` e passa a copiar blocos:

* `NoG::Bloco { cid, flags, metadado, palavras: Box<[i64]>, refs: Vec<(u32, ValG)> }`: o corpo
  copiado cru; `refs` são as posições que são referência (pelo mapa em `INSTANCIA`, pelas palavras
  pares não nulas em `REFS`, nenhuma em `BRUTO`). O destino aloca o bloco com o mesmo cid, `flags`
  (sem `CARTOES` se não precisar; o destino recalcula) e palavras, e reescreve as posições `refs`.
* Especiais: estático → `ValG::Estatico(h)` (o mesmo endereço em todo isolado do processo);
  `ANEXO` → `NoG::Acumulador(Vec<u16>)` e `NoG::ProgramaRe { padrao, flags }` (recompilado no
  destino); lista tipada `EXTERNO` → os bytes copiados numa interna (a VM copia a externa para
  memória nova, `object_graph_copy.cc:331-343`); visão → base copiada e `dados` recalculado;
  `TransferableTypedData`, `SendPort`, `Capability`, constantes por getter e tear-offs como hoje.
* Mesmo isolado: strings e estáticos por identidade (hoje, `RT/portas.rs:105-111`).
* `Portavel`/`Dart_PostCObject` (`RT/ffi_api_nativa.rs`, `RT/portas.rs:317-476`) não tocam o heap e
  só mudam no `para_grafo`.

### 2.13 JIT e recarga

* `migrar_instancias` (`RT/heap.rs:3989-4015`) só toca blocos `INSTANCIA` cujo cid está no plano; os
  cids do runtime (< 128) nunca estão (`crates/jit/src/migracao.rs` só planeja classes do programa;
  o runtime recusa um plano com cid < 128).
* Os cids fixos são iguais em toda geração; a numeração das demais continua a de J03
  (`EN/context.rs:398-418`).
* Os caches da área (seletor, RTI, literal) continuam sendo esvaziados na publicação
  (`RT/gc_raizes.rs:300-310`); a chave do cache RTI é a mesma (metadado ou `−16 − cid`, lida do
  cabeçalho).
* Sem estáticos no JIT (§2.11). Os estáticos do runtime (`bool`) moram no próprio processo.

### 2.14 `dart:io`, FFI, TLS, zlib

* Não mover garante que `dados` de uma lista tipada e as unidades de uma string têm endereço fixo
  enquanto o objeto vive. Regra para o runtime: um ponteiro para dentro de um objeto vale até a
  próxima alocação **se** o objeto não está enraizado, e enquanto ele estiver enraizado.
* `tls.rs` deixa de fazer `mem::take` do `Armazenamento` (763-777): processa sobre as fatias dos
  objetos, que não se movem nem crescem.
* `com_memoria` (`RT/ffi.rs:262-282`) e `dartforge_ffi_endereco_do_composto` (814-829) usam
  `dados`.
* E/S assíncrona continua copiando para buffers do runtime (§1.9); zero-cópia em escrita síncrona
  fica possível, mas não faz parte desta especificação.
* `asTypedList` cria a lista `EXTERNO` (§2.5); o `finalizer:` continua pelo `NativeFinalizer`
  (`sdk_nativo/ffi/ffi_native_finalizer_patch.dart:54-64`).

### 2.15 RTI

* O metadado de todo objeto está no cabeçalho (`metadado`, u32 `id + 1`), inclusive listas,
  closures e o que era slot; somem `Heap::metadados`, `metadado_e_valor` e o caso de slot de
  `metadado`/`set_metadado`.
* `chave_do_valor` (`RT/tipos.rs:133-167`) fica uniforme: `metadado` se não é 0, senão
  `−16 − cid`, para todo objeto; `null` e `Smi` pelos cids 1 e 2. Some a tabela de chaves por
  variante (−1…−4).
* `tipo_do_valor` (781-841) despacha pelo cid.
* A forma compacta de uma lista sai do `E` reificado quando `rti_definir` grava o tipo
  (`ajustar_forma_da_lista`, 928): troca `ELEMENTO`/`FORMA` do armazenamento, convertendo no lugar
  os elementos que já existem (`Smi` → `i64`, `_Double` → bits; nenhuma alocação).

### 2.16 Fixa, imutável e forma compacta

* **A classe diz**: `_List` fixa, `_ImmutableList` imutável, `_GrowableList` expansível. Somem
  `MARCA_FIXA`, `MARCA_IMUTAVEL`, `ClasseDoSlot`, `marcar_fixa`, `marcar_imutavel` e o
  `imutaveis_do_espaco`.
* **O cid não muda.** O que hoje marca depois de criar passa a criar da classe certa:
  `List_allocate` → `_List`; `ImmutableList_from` → `_ImmutableList`;
  `Internal_makeListFixedLength` → um `_List` novo com os elementos (como a VM);
  `Internal_makeFixedListUnmodifiable` e `dartforge_collection_mark_unmodifiable` → um
  `_ImmutableList` novo com os elementos (a VM troca o cid no lugar, `Array::MakeImmutable`; aqui
  não, porque `df.classe` usa `!invariant.load`). Os chamadores do SDK usam o valor devolvido
  (`List.unmodifiable`, as constantes); P3 confere cada um.
* **Forma compacta** (`ELEMENTO`) só em `_List`/`_ImmutableList` (a `_GrowableList` herda a do
  armazenamento). É mutável e não é lida com `!invariant.load`. Descompactar (gravar um valor que a
  forma não guarda) aloca caixas: na `_GrowableList`, monta um armazenamento `REFS` novo enraizado
  e troca `dados`; na `_List`, monta uma `_List` geral temporária enraizada com as caixas e depois,
  sem alocação no meio, copia as palavras e troca `flags`. O código gerado grava direto
  (`GravacaoNativa`) quando o cid é 8 ou 10 e `ELEMENTO` casa com a representação do valor; o bit
  `gravavel` do `CabecalhoDeLista` deixa de existir.

### 2.17 O que sobra e o que some

* **Some** do runtime: `enum Value` e `Marcador`/`VALOR_OBJETO`; `Heap::slots`, `metadados`,
  `free`, `marks`, `idade`, `slots_jovens`, `slots_lembrados`, `bytes_do_slot`,
  `hashes_de_texto`, `classes`, `cids_do_runtime`, `caixas_bool`, `imutaveis_do_espaco`,
  `marcador`; `get`, `try_get`, `get_mut`, `allocate`, `allocate_linked`, `normalizar`,
  `key_equal`, `create_*`/`list_*`/`map_*`/`set_*`/`cell_*`/`environment_get`/`closure_parts`;
  `TaggedValue`/`ValueTag` no heap; `Elementos`, `CabecalhoDeLista`, `FormaDeLista`,
  `Armazenamento`, `CabecalhoTipado`, `CabecalhoDeClosure` e os vazios deles; `ClasseDoSlot`,
  `Contexto::classes`/`n_classes`; `iteracoes_ativas`, `origens`.
* **Some** do emissor: o ramo de slot de `@df.classe`; `@df.env_ref`; os caches de texto
  (`EN/lower/textos.rs:121-188`) e de cabeçalho tipado (`EN/lower/tipados.rs:517-568`); o cache de
  literal no ponto de uso no AOT; a ABI de pares `(bits, tag)` de `AllocList`/`AllocRecord`/
  `AllocEnv`/`AllocCell` (os valores vão como palavras); 35 externs de acesso (§3.7).
* **Some** o caminho sem SDK da fonte: `DARTFORGE_SDK_DA_FONTE=0`, `EN/lower/sdk_por_nome.rs`
  (a decisão 7.1.1 do proprietário já o previa), os ramos `!sdk_da_fonte` do lowering e os externs
  legados de `RT/strings.rs` e `RT/colecoes.rs`. `Value::Map`, `Value::Set`, `Value::RegExp` e
  `Value::Match` só existem nele.
* **Fica:** `Texto` como tipo de construção do runtime (dono das unidades antes de alocar;
  `TextoMut`); `Campo = (i64, bool)` dos campos `INSTANCIA`; `Obj` (vista de objeto `INSTANCIA`);
  `Heap::frames` (quadros de raízes do runtime); as tabelas por handle (fracas, efêmeros,
  finalizáveis, anexos, `late`, `literais`, `permanentes`, `constantes`, `tearoffs`,
  `enum_values`, `globais`); `OBJETO_VAZIO`; o corpo de fora (só `INSTANCIA`).
* **Nada do `enum Value` sobra.**

---

## 3. A API nova

As assinaturas abaixo são o contrato entre os pacotes (§4). Um pacote não muda assinatura de
outro; se precisar, o pedido vai ao dono e a mudança é registrada neste documento.

### 3.1 `crates/runtime/src/layout.rs` (P0): o contrato de layout

Módulo novo, **sem dependências**, compilado no runtime (`pub mod layout` em `lib.rs`; `mod
layout` no `RUNTIME_MAIN`; aceito por `conferir_lista` do `build.rs`) e usado pelo emissor como
`dartforge_runtime::layout` (o `emit_native` já depende do runtime, `EN/../Cargo.toml`). Toda
constante numérica que o código gerado usa sai daqui; o emissor não escreve deslocamento à mão.

```rust
pub type Ref = i64;

/// O cabeçalho de todo bloco (16 bytes, `#[repr(C)]`, os asserts de hoje).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Cabecalho { pub estado: u8, pub flags: u8, pub n: u16, pub class_id: i32, pub mapa: u32, pub metadado: u32 }

pub const TAMANHO_DO_CABECALHO: usize = 16;
pub const DESLOCAMENTO_DO_HANDLE: i64 = 2;
pub const PAGINA: usize = 64 * 1024;
pub const CABECA_DA_PAGINA: usize = PAGINA / 64;
pub const MAIOR_CLASSE: usize = 64;
pub const CLASSES_MEDIAS: [usize; 24] =
    [70, 82, 98, 110, 124, 142, 166, 199, 222, 250, 286, 334, 401, 446, 502, 574, 670, 804, 894, 1006, 1150, 1342, 1610, 2014];
pub const MAIOR_MEDIA: usize = 2014;
pub const N_CLASSES: usize = 1 + MAIOR_CLASSE + CLASSES_MEDIAS.len(); // 89; o índice 0 não é usado
pub const TLAB_N: usize = 16;
pub const CAMPOS_EM_LINHA: usize = 4096;
pub const ELEMENTOS_POR_CARTAO: usize = 32;
pub const ELEMENTOS_POR_PALAVRA_DE_CARTAO: usize = 2048;
pub const PRIMEIRO_CID_LIVRE: i64 = 128;

pub mod estado {
    pub const LIVRE: u8 = 0; pub const JOVEM: u8 = 1; pub const VELHO: u8 = 3;
    pub const LEMBRADO: u8 = 4; pub const PERMANENTE: u8 = 5;
}
pub mod flags {
    pub const FORA: u8 = 0x01;
    pub const FORMA: u8 = 0x06; pub const INSTANCIA: u8 = 0x00; pub const BRUTO: u8 = 0x02; pub const REFS: u8 = 0x04;
    pub const CARTOES: u8 = 0x08;
    pub const ANEXO: u8 = 0x10;
    pub const ELEMENTO: u8 = 0x60; pub const ELEMENTO_INT: u8 = 0x20; pub const ELEMENTO_DOUBLE: u8 = 0x40; pub const ELEMENTO_BOOL: u8 = 0x60;
    pub const EXTERNO: u8 = 0x80;
}
pub mod cid {
    pub const NULL: i32 = 1; pub const SMI: i32 = 2; pub const MINT: i32 = 3; pub const DOUBLE: i32 = 4; pub const BOOL: i32 = 5;
    pub const ONE_BYTE_STRING: i32 = 6; pub const TWO_BYTE_STRING: i32 = 7;
    pub const LIST: i32 = 8; pub const IMMUTABLE_LIST: i32 = 9; pub const GROWABLE_LIST: i32 = 10;
    pub const CLOSURE: i32 = 11; pub const RECORD: i32 = 12; pub const CONTEXTO: i32 = 13; pub const CELULA: i32 = 14;
    pub const ACUMULADOR_DE_TEXTO: i32 = 15; pub const PROGRAMA_DE_REGEXP: i32 = 16;
    pub const SEND_PORT: i32 = 17; pub const CAPABILITY: i32 = 18;
    pub const FLOAT32X4: i32 = 19; pub const INT32X4: i32 = 20; pub const FLOAT64X2: i32 = 21;
    pub const PRIMEIRA_TIPADA: i32 = 22; pub const PRIMEIRA_VISAO: i32 = 36; pub const PRIMEIRA_VISAO_IMUTAVEL: i32 = 50;
    pub const BYTE_DATA_VIEW: i32 = 64; pub const UNMODIFIABLE_BYTE_DATA_VIEW: i32 = 65;
    /// `(cid, biblioteca, classe)` de toda classe do SDK com cid fixo, na ordem: a fonte da
    /// numeração do emissor (`context.rs`) e da conferência de ABI.
    pub const DO_SDK: &[(i32, &str, &str)] = &[/* 1 Null … 65 _UnmodifiableByteDataView, §2.4 */];
    pub const fn tipada(tipo: u8) -> i32 { PRIMEIRA_TIPADA + tipo as i32 }
    pub const fn visao(tipo: u8, imutavel: bool) -> i32 { (if imutavel { PRIMEIRA_VISAO_IMUTAVEL } else { PRIMEIRA_VISAO }) + tipo as i32 }
}
/// Deslocamentos a partir do bloco (o handle é bloco + 2).
pub mod desl {
    pub const ESTADO: usize = 0; pub const FLAGS: usize = 1; pub const N: usize = 2; pub const CLASSE: usize = 4;
    pub const MAPA: usize = 8; pub const HASH_DO_TEXTO: usize = 8; pub const METADADO: usize = 12; pub const CORPO: usize = 16;
    pub const COMPRIMENTO: usize = 16;                    // strings, _List, _GrowableList, listas tipadas, visões
    pub const UNIDADES: usize = 24; pub const ELEMENTOS: usize = 24; pub const VALOR: usize = 16;
    pub const DADOS: usize = 24; pub const BYTES_INTERNOS: usize = 32;
    pub const BASE_DA_VISAO: usize = 32; pub const DESLOCAMENTO_DA_VISAO: usize = 40;
    pub const EXPANSIVEL_DADOS: usize = 24;
    pub const CLOSURE_CODIGO: usize = 16; pub const CLOSURE_CONTEXTO: usize = 24; pub const CLOSURE_TIPADO: usize = 32; pub const CLOSURE_ABI: usize = 40;
    pub const RECORD_FORMA: usize = 16; pub const RECORD_CAMPOS: usize = 24;
    pub const ANEXO: usize = 16;
}
/// Deslocamentos do `Contexto` da thread (§3.4).
pub mod contexto {
    pub const PENDENTE: usize = 0; pub const TOPO: usize = 8; pub const AREAS: usize = 16; pub const N_AREAS: usize = 24;
    pub const INTERRUPCAO: usize = 32; pub const VAZIOS: usize = 40; pub const REGISTRADAS: usize = 48; pub const N_REGISTRADAS: usize = 56;
    pub const TLAB: usize = 64; // cursor em TLAB + 16·w, fim em TLAB + 16·w + 8, w em 1..=TLAB_N
    pub const SUBTIPOS: usize = 336; pub const N_SUBTIPOS: usize = 344; pub const LARGURA_SUBTIPOS: usize = 352;
}
pub mod smi { /* o módulo de hoje (RT/heap.rs:2686-2714), movido */ }
pub fn e_objeto(h: Ref) -> bool;                                         // h & (7 | i64::MIN) == 2
pub const fn palavras_do_mapa(n: usize) -> usize;                        // de hoje
pub const fn capacidade(n: usize) -> usize;                              // de hoje
pub const fn palavras_de_instancia(n: usize) -> usize;                   // capacidade(n) + palavras_do_mapa(n)
pub const fn palavras_de_texto(len: usize, dois: bool) -> usize;         // 1 + ⌈len·(1|2)/8⌉
pub const fn palavras_de_lista(len: usize) -> usize;                     // 1 + len (+ ⌈len/2048⌉ se passa de MAIOR_MEDIA)
pub const fn palavras_de_tipada(len: usize, tamanho_do_elemento: usize) -> usize; // 2 + ⌈len·t/8⌉
pub const fn tem_cartoes(len: usize) -> bool;                            // 1 + len > MAIOR_MEDIA
pub const fn classe_de_tamanho(palavras: usize) -> Option<usize>;        // None: objeto grande
pub const fn palavras_da_classe(classe: usize) -> usize;
pub const fn palavra_do_cabecalho(estado: u8, flags: u8, palavras: usize, cid: i32) -> u64; // estado | flags<<8 | n<<16 | cid<<32
pub fn hash_de_texto(unidades: impl IntoIterator<Item = u16>) -> u32;   // o StringHasher da VM, 30 bits, nunca 0
```

### 3.2 O `Heap` (P0: `crates/runtime/src/heap.rs` e `crates/runtime/src/espaco.rs`)

`espaco.rs` (módulo novo) recebe `EspacoDeObjetos`, `Pagina`, `MapaDePaginas`,
`ReservaDePaginas`, as regiões grandes, os cartões e a varredura; `heap.rs` fica com o `Heap`
(coleta, raízes, tabelas laterais), o `Contexto` e a pilha-sombra. Em ambos, a API pública é:

```rust
pub use crate::layout::{Cabecalho, Ref, e_objeto, smi};
pub type Campo = (i64, bool);

/// Um valor sem representação de heap decidida: substitui `TaggedValue` nas APIs do runtime.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Valor { Ref(Ref), Int(i64), Double(f64), Bool(bool) }

impl Heap {
    pub fn new(stress: bool) -> Self;
    pub fn do_isolado(stress: bool) -> Self;

    // Alocação: bloco zerado, estado JOVEM; coleta antes se um gatilho pede (as referências
    // que o chamador segura precisam de raiz, como hoje).
    pub fn alocar(&mut self, cid: i32, palavras: usize, flags: u8) -> Ref;
    pub fn alocar_instancia(&mut self, cid: i32, campos: usize) -> Ref;      // o alocar_objeto de hoje
    pub fn novo_objeto(&mut self, cid: i32, campos: &[Campo]) -> Ref;        // de hoje
    pub fn reabastecer_tlab(&mut self, palavras: usize);                     // de hoje, por palavras

    // Cabeçalho e corpo. Pânico N4 (mensagens de hoje) para null, Smi, morto ou inválido.
    pub fn cabecalho(&self, h: Ref) -> &Cabecalho;
    pub fn classe(&self, h: Ref) -> i32;                                     // 1 para 0, 2 para Smi
    pub fn e_objeto_vivo(&self, h: Ref) -> bool;                             // sem pânico
    pub fn e_estatico(&self, h: Ref) -> bool;
    pub fn palavras(&self, h: Ref) -> &[i64];                                // o corpo inteiro
    pub fn palavras_mut(&mut self, h: Ref) -> &mut [i64];                    // sem barreira: BRUTO, ou antes de publicar
    pub fn bytes(&self, h: Ref) -> &[u8];
    pub fn bytes_mut(&mut self, h: Ref) -> &mut [u8];
    pub fn definir_flags(&mut self, h: Ref, mascara: u8, valor: u8);         // só FORMA/ELEMENTO de _List

    // INSTANCIA (de hoje; recusam outro formato).
    pub fn objeto(&self, h: Ref) -> Option<Obj<'_>>;
    pub fn definir_campo(&mut self, h: Ref, i: usize, bits: i64, e_ref: bool);
    pub fn garantir_campos(&mut self, h: Ref, n: usize);

    // REFS: palavra ≥ 1 do corpo, com barreira (e cartão).
    pub fn gravar_ref(&mut self, h: Ref, palavra: usize, v: Ref);
    pub fn gravar_refs(&mut self, h: Ref, palavra: usize, v: &[Ref]);        // cópia em bloco, uma barreira

    // Barreira explícita (para quem grava por palavras_mut num objeto publicado).
    pub fn lembrar(&mut self, h: Ref);
    pub fn lembrar_elemento(&mut self, h: Ref, i: usize);

    // Anexos nativos (ANEXO): `ptr` em b+16, `soltar(ptr)` quando o dono morre.
    pub unsafe fn anexar(&mut self, h: Ref, soltar: unsafe fn(*mut u8), ptr: *mut u8, bytes: usize);
    pub fn anexo(&self, h: Ref) -> *mut u8;
    pub fn contar_externos(&mut self, delta: isize);

    // Metadado e identidade.
    pub fn metadado(&self, h: Ref) -> i64;
    pub fn set_metadado(&mut self, h: Ref, v: i64);
    pub fn hash_de_identidade(&self, h: Ref) -> Option<i64>;

    // Raízes, constantes, late, finalização, coleta, recarga: as assinaturas de hoje
    // (push_frame, push_frame_with_slots, set_root, root, pop_frame, set_global_root,
    // mover_raiz_global, soltar_raiz_global, set_raiz_do_runtime, marcar_permanente,
    // marcar_constante, getter_da_constante, e_permanente, codigo_do_tearoff, enum_value,
    // late_inicializado, marcar_late, desmarcar_late, collect, stats, migrar_instancias,
    // encerrar_finalizadores, os campos pub fracas/efemeros/finalizaveis/anexos/
    // finalizacoes_prontas), com `i64` de class_id trocado por `i32` onde é cid.
}

/// Os estáticos de `bool` (seção da imagem, estado PERMANENTE).
#[unsafe(no_mangle)] pub static dartforge_falso: [u64; 3];
#[unsafe(no_mangle)] pub static dartforge_verdadeiro: [u64; 3];
```

Some: `get`, `try_get`, `get_mut`, `allocate`, `allocate_linked`, `normalizar`, `como_ref`
(vai para P2), `int_de_ref` (P2), `key_equal`, `string_equal`/`string_concat`/`texto`/
`string_literal`/`hash_de_texto` (P1), `create_*`, `list_*`, `map_*`, `set_*`, `cell_*`,
`environment_get`, `closure_parts`, `metadado_e_valor`, `marcar_fixa`, `e_fixa`,
`marcar_imutavel`, `e_imutavel`, `classe_do_slot`, `definir_cids_do_runtime`,
`campos_de_objeto` (fica só para `INSTANCIA`), `caixa_bool`.

### 3.3 Vistas por pacote

Cada pacote escreve um módulo `impl Heap` próprio sobre a API de §3.2 (sem campo privado do
`Heap`): `textos.rs` (P1), `caixas.rs` (P2), `listas.rs` (P3), `tipadas.rs` (P4). P0 os registra em
`lib.rs`, no `RUNTIME_MAIN` e em `conferir_lista`.

**P1, `crates/runtime/src/textos.rs`** (o `Texto`, o `TextoMut`, o `IterUnidades` e
`empurrar_ponto` saem de `RT/heap.rs:97-560` para cá; `heap.rs` os reexporta):

```rust
#[derive(Clone, Copy)]
pub enum TextoRef<'a> { Um(&'a [u8]), Dois(&'a [u16]) }
impl<'a> TextoRef<'a> {
    pub fn len(self) -> usize; pub fn is_empty(self) -> bool; pub fn e_um_byte(self) -> bool;
    pub fn unidade(self, i: usize) -> u16; pub fn unidades(self) -> IterUnidades<'a>;
    pub fn fatia(self, inicio: usize, fim: usize) -> TextoRef<'a>;
    pub fn para_texto(self) -> Texto; pub fn para_string(self) -> String;
    pub fn para_wtf8(self) -> Vec<u8>; pub fn para_utf8_da_vm(self) -> Vec<u8>; pub fn pontos(self) -> Vec<u32>;
    pub fn procurar(self, padrao: TextoRef<'_>, desde: usize) -> Option<usize>;
    pub fn procurar_ultimo(self, padrao: TextoRef<'_>, ate: usize) -> Option<usize>;
    pub fn coincide_em(self, padrao: TextoRef<'_>, i: usize) -> bool;
    pub fn comparar(self, outro: TextoRef<'_>) -> std::cmp::Ordering;
    pub fn hash_vm(self) -> u32;
}
// PartialEq entre TextoRef, com Texto e com str; Display; Debug.
impl Texto { pub fn vista(&self) -> TextoRef<'_>; /* construtores de hoje */ }

impl Heap {
    pub fn texto(&self, h: Ref) -> Option<TextoRef<'_>>;                 // None se não é string
    pub fn e_texto(&self, h: Ref) -> bool;
    pub fn novo_texto(&mut self, len: usize, dois: bool) -> Ref;          // unidades zeradas
    pub fn unidades_um_mut(&mut self, h: Ref) -> &mut [u8];               // só antes de publicar
    pub fn unidades_dois_mut(&mut self, h: Ref) -> &mut [u16];
    pub fn alocar_texto(&mut self, t: TextoRef<'_>) -> Ref;                // forma canônica (Um se cabe)
    pub fn alocar_str(&mut self, s: &str) -> Ref;
    pub fn texto_de_wtf8(&mut self, bytes: &[u8]) -> Ref;
    pub fn hash_de_texto(&self, h: Ref) -> Option<u32>;                   // calcula e grava em `mapa`
    pub fn textos_iguais(&self, a: Ref, b: Ref) -> bool;
    pub fn string_literal(&mut self, t: TextoRef<'_>) -> Ref;             // internado (JIT, runtime)
}
```

**P2, `crates/runtime/src/caixas.rs`:**

```rust
#[derive(Clone, Copy, Debug)]
pub struct ClosureRef { pub codigo: i64, pub contexto: Campo, pub tipado: i64, pub abi: i64 }
impl Heap {
    pub fn caixa_int(&mut self, v: i64) -> Ref;                  // Smi ou _Mint
    pub fn caixa_double(&mut self, v: f64) -> Ref;
    pub fn caixa_bool(v: bool) -> Ref;                           // uma das duas estáticas
    pub fn como_ref(&mut self, v: Valor) -> Ref;
    pub fn valor(&self, r: Ref) -> Valor;                        // desencaixa Smi/_Mint/_Double/bool
    pub fn int_de(&self, r: Ref) -> Option<i64>;
    pub fn double_de(&self, r: Ref) -> Option<f64>;
    pub fn bool_de(&self, r: Ref) -> Option<bool>;
    pub fn identico(&self, a: Ref, b: Ref) -> bool;
    pub fn nova_celula(&mut self, valor: Campo) -> Ref;
    pub fn celula(&self, h: Ref) -> Campo;
    pub fn gravar_celula(&mut self, h: Ref, valor: Campo);
    pub fn novo_contexto(&mut self, capturas: &[Campo]) -> Ref;
    pub fn captura(&self, h: Ref, i: usize) -> Campo;
    pub fn nova_closure(&mut self, codigo: i64, contexto: Campo, tipado: i64, abi: i64) -> Ref;
    pub fn closure(&self, h: Ref) -> Option<ClosureRef>;
    pub fn tearoff(&mut self, codigo: i64) -> Ref;              // canônico (tabela tearoffs)
    pub fn novo_record(&mut self, campos: &[Ref]) -> Ref;
    pub fn record(&self, h: Ref) -> Option<&[i64]>;              // os campos
}
```

**P3, `crates/runtime/src/listas.rs`:**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elemento { Geral, Int, Double, Bool }
#[derive(Clone, Copy)]
pub enum ElementosRef<'a> { Geral(&'a [i64]), Int(&'a [i64]), Double(&'a [f64]), Bool(&'a [i64]) }
impl Heap {
    pub fn nova_lista(&mut self, cid: i32, len: usize, e: Elemento) -> Ref;          // _List/_ImmutableList zerada
    pub fn nova_expansivel(&mut self, len: usize, capacidade: usize, e: Elemento) -> Ref;
    pub fn e_lista(&self, h: Ref) -> bool;                                           // cid 8, 9, 10
    pub fn lista_len(&self, h: Ref) -> usize;
    pub fn lista_capacidade(&self, h: Ref) -> usize;
    pub fn lista_forma(&self, h: Ref) -> Elemento;
    pub fn lista_elementos(&self, h: Ref) -> ElementosRef<'_>;                       // cortada no comprimento
    pub fn lista_get(&self, h: Ref, i: usize) -> Valor;
    pub fn lista_get_ref(&mut self, h: Ref, i: usize) -> Ref;                        // encaixota
    pub fn lista_set(&mut self, h: Ref, i: usize, v: Valor);                         // descompacta se preciso
    pub fn lista_push(&mut self, h: Ref, v: Valor);                                   // _GrowableList
    pub fn lista_definir_len(&mut self, h: Ref, len: usize);
    pub fn lista_dados(&self, h: Ref) -> Ref;
    pub fn lista_definir_dados(&mut self, h: Ref, dados: Ref);
    pub fn lista_reservar(&mut self, h: Ref, capacidade: usize);
    pub fn lista_ajustar_forma(&mut self, h: Ref, e: Elemento);
    pub fn lista_copiar(&mut self, de: Ref, inicio: usize, para: Ref, destino: usize, n: usize);
    pub fn lista_fixa_de(&mut self, h: Ref) -> Ref;                                  // cópia _List
    pub fn lista_imutavel_de(&mut self, h: Ref) -> Ref;                              // cópia _ImmutableList
}
```

**P4, `crates/runtime/src/tipadas.rs`:**

```rust
#[derive(Clone, Copy, Debug)]
pub struct TipadaRef { pub cid: i32, pub tipo: u8, pub len: usize, pub dados: *mut u8, pub bytes: usize,
                       pub imutavel: bool, pub externa: bool, pub base: Option<Ref>, pub deslocamento: usize }
impl Heap {
    pub fn nova_tipada(&mut self, tipo: u8, len: usize) -> Ref;
    pub fn tipada_externa(&mut self, tipo: u8, endereco: *mut u8, len: usize) -> Ref;
    pub fn nova_visao(&mut self, cid: i32, base: Ref, deslocamento: usize, len: usize) -> Ref;
    pub fn e_tipada(&self, h: Ref) -> bool;                                          // 22..=65
    pub fn tipada(&self, h: Ref) -> Option<TipadaRef>;
    pub fn bytes_da_tipada(&self, h: Ref) -> Option<&[u8]>;
    pub fn bytes_da_tipada_mut(&mut self, h: Ref) -> Option<&mut [u8]>;              // None se não modificável
    pub fn novo_simd(&mut self, cid: i32, pistas: [u8; 16]) -> Ref;
    pub fn simd(&self, h: Ref) -> Option<[u8; 16]>;
}
```

O acumulador do `StringBuffer` (P1, fragmento `nativos_strings.rs`) e o programa de `RegExp` (P1,
`regexp.rs`) usam `alocar` + `anexar` direto: os tipos deles (`Vec<u16>`, `ProgramaRe`) são dos
fragmentos, que o módulo `heap` não enxerga.

### 3.4 O `Contexto`

Sai `classes`/`n_classes` (336/344); `subtipos`, `n_subtipos` e `largura_subtipos` passam a
336/344/352. O resto não muda (`layout::contexto`). A TLAB passa a ser indexada por palavras
(`tlab[w]`, `w` em `1..=16`; a posição 0 fica sem uso).

### 3.5 Helpers `df.*` do IR

Todos `define internal … alwaysinline`, impressos no prelúdio de cada módulo por
`emit_runtime_decls` (`EN/llvm/mod.rs:296-322`). Os que precisam do contexto chamam
`@dartforge_contexto()` (`memory(none) … speculatable`; o LLVM o une ao `%ctx` da função).

| Helper | Assinatura | Semântica | Dono |
|---|---|---|---|
| `@df.classe` | `i64 (i64 %h)` | `0` → 1; ímpar → 2; senão `sext (load i32 [h+2], !invariant.load)`. Sem chamada. | P0 |
| `@df.e_objeto` | `i1 (i64 %h)` | `(h & 0x8000000000000007) == 2` | P0 |
| `@df.alocar` | `i64 (i64 %cabecalho, i64 %w)` | `w ≤ 16`: TLAB `[ctx+64+16w, ctx+72+16w)`, avança `16+8w`, grava a palavra 0; esgotada ou `w > 16`: `@dartforge_alocar(cid, w, flags)` | P0 |
| `@df.barreira` | `void (i64 %o, i64 %v)` | §2.7, peso 1:9 no ramo lento | P0 |
| `@df.barreira_elemento` | `void (i64 %o, i64 %i, i64 %v)` | §2.7 com cartões | P0 |
| `@df.subclasse` | `i8 (i64 %cid, i64 %alvo)` | a de hoje com os deslocamentos novos | P0 |
| `@df.seletor`, `@df.obter_area`, `@df.preparar_area` | as de hoje | inalterados | P0 |
| `@df.texto_len` | `i64 (i64 %s)` | `load i64 [s+14]` (o chamador sabe que é `String`) | P1 |
| `@df.texto_unidade` | `i64 (i64 %s, i64 %i)` | sem conferir limites: cid 6 → `zext load i8 [s+22+i]`, 7 → `zext load i16 [s+22+2i]` | P1 |
| `@df.texto_alocar` | `i64 (i64 %len, i1 %dois)` | `w = palavras_de_texto`; `@df.alocar` com `JOVEM | BRUTO<<8 | w<<16 | cid<<32`; grava `len`; `w > 16` → `@dartforge_texto_novo` | P1 |
| `@df.texto_gravar` | `void (i64 %s, i64 %i, i64 %u)` | o `writeInto*String`: `store i8`/`i16` pelo cid | P1 |
| `@df.texto_hash` | `i64 (i64 %s)` | `load i32 [s+6]`; 0 → `@dartforge_texto_hash(s)` | P1 |
| `@df.texto_igual` | `i1 (i64 %a, i64 %b)` | `a == b`; cids iguais, comprimentos iguais, hashes (se os dois calculados) iguais; senão `@dartforge_texto_iguais` | P1 |
| `@df.caixa_int` | `i64 (i64 %v)` | `Smi` na faixa; senão `_Mint` por `@df.alocar` (w = 1) | P2 |
| `@df.desencaixa_int` | `i64 (i64 %r)` | ímpar → `ashr 1`; objeto de cid 3 → `[r+14]`; senão `@dartforge_unbox_int` (lança) | P2 |
| `@df.caixa_double` | `i64 (double %v)` | `_Double` por `@df.alocar` (w = 1) | P2 |
| `@df.desencaixa_double` | `double (i64 %r)` | cid 4 → `[r+14]`; senão `@dartforge_unbox_double` | P2 |
| `@df.caixa_bool` | `i64 (i1 %v)` | `select` entre `@dartforge_verdadeiro+2` e `@dartforge_falso+2` | P2 |
| `@df.desencaixa_bool` | `i1 (i64 %r)` | `r == verdadeiro`; nem uma nem outra → `@dartforge_unbox_bool` | P2 |
| `@df.identico` | `i1 (i64 %a, i64 %b)` | §2.10 | P2 |
| `@df.lista_len` | `i64 (i64 %l)` | `load i64 [l+14]` (o mesmo deslocamento em `_List`, `_ImmutableList` e `_GrowableList`) | P3 |
| `@df.lista_armazenamento` | `i64 (i64 %l)` | cid 10 → `[l+22]`; senão `l` | P3 |
| `@df.lista_elemento` | `ptr (i64 %a, i64 %i)` | `a + 22 + 8i` (sobre o armazenamento) | P3 |
| `@df.lista_forma` | `i8 (i64 %a)` | `load i8 [a−1] & 0x66` | P3 |
| `@df.tipada_len` | `i64 (i64 %t)` | `load i64 [t+14]` | P4 |
| `@df.tipada_dados` | `ptr (i64 %t)` | `load ptr [t+22]` | P4 |
| `@df.simd_caixa` | `i64 (<2 x i64> %v, i64 %cid)` | `@df.alocar` (w = 2) e grava as pistas | P4 |

Cada pacote escreve os seus num arquivo próprio do emissor (§4) que exporta `pub const
AJUDANTES: &str`; `emit_runtime_decls` (P0) concatena os quatro.

### 3.6 Externs novas ou mudadas (`EN/llvm/externs.rs`, P0)

```llvm
declare i64 @dartforge_alocar(i64, i64, i64) nounwind                   ; (cid, palavras, flags): ALOCA_SEM_LANCAR
declare void @dartforge_lembrar(i64) nounwind                           ; de hoje
declare i64 @dartforge_texto_novo(i64, i64) nounwind                    ; (len, dois): ALOCA_SEM_LANCAR
declare i64 @dartforge_texto_hash(i64) nounwind willreturn              ; grava o cabeçalho; {aloca: false, lanca: false}
declare i8 @dartforge_texto_iguais(i64, i64) memory(read) nounwind willreturn  ; {false, false}
declare i64 @dartforge_lista_nova(ptr, i64, i64, i64) nounwind          ; (palavras, n, cid, forma): literal; ALOCA_SEM_LANCAR
declare i64 @dartforge_lista_acrescentar(i64, i64) nounwind             ; (lista, Ref): o add que cresce; ALOCA_SEM_LANCAR
declare i64 @dartforge_record_novo(ptr, i64) nounwind                   ; (Refs, n): ALOCA_SEM_LANCAR
@dartforge_verdadeiro = external constant [3 x i64]
@dartforge_falso = external constant [3 x i64]
; mantidas como caminho lento que lança TypeError:
declare i64 @dartforge_unbox_int(i64)
declare double @dartforge_unbox_double(i64)
declare i8 @dartforge_unbox_bool(i64)
; mantida só no JIT (literal internado):
declare i64 @dartforge_string_new(ptr, i64)
```

`dartforge_registrar_cids(ptr, i64)` fica, com semântica de conferência (aborta se diferente de
`layout::cid::DO_SDK`).

### 3.7 Externs e símbolos que somem

* **Acesso à representação antiga** (o código gerado passa a ler o bloco):
  `dartforge_texto_dados`, `_texto_len`, `_texto_na_falha`, `_lista_cabecalho`,
  `_lista_len_gravavel`, `_lista_len_ou_menos1`, `_lista_ref`, `_lista_add_escalar` (vira
  `_lista_acrescentar`), `_closure_cabecalho`, `_closure_code`, `_closure_env`,
  `_closure_tipada`, `_env_dados`, `_env_new`, `_env_get`, `_env_get_ref`, `_cell_new`,
  `_cell_get_bits`, `_cell_get_tag`, `_cell_get_ref`, `_cell_set`, `_box_int`, `_box_double`,
  `_box_bool`, `_typed_len`, `_typed_ptr`, `_typed_cabecalho`, `_typed_cabecalho_na_falha`,
  `_simd_caixa`, `_object_campos`, `_record_new` (vira `_record_novo`), `_record_len`,
  `_record_get_ref`, `_list_new` (vira `_lista_nova`), `_collection_is_unmodifiable`.
  `dartforge_value_class` deixa de ser chamado pelo código gerado (fica no runtime como função
  interna trivial). `dartforge_closure_new`/`_new_tipada`/`_nova_direta` e `dartforge_tearoff`
  podem ficar como caminho lento; a criação em linha (`@df.alocar` + gravações) é de P2.
* **Legado sem SDK da fonte** (saem com `lower/sdk_por_nome.rs`): `dartforge_list_len`,
  `_list_get_bits`, `_list_get_tag`, `_list_set`, `_list_push`, `_list_get_ref`,
  `_list_first*`/`_last*`/`_single*`, `_generic_len`, `_list_join`, `_list_new_empty`,
  `_list_reversed`, `_list_sublist`, `_list_remove_at`, `_list_filled`, `_iteration_begin`/`_end`,
  `_iteravel_get_ref`/`_bits`, todos os `_map_*` (10) e `_set_*` (4), os `_string_*` do lowering
  por nome (≈35: `_string_len`, `_code_unit_at`, `_code_units`, `_runes`, `_to_upper`, `_repeat`,
  `_substring`, `_from_char_code(s)`, `_index_of`, `_last_index_of`, `_split`, `_contains`,
  `_replace_all`, `_pad_left`/`_right`, `_trim*`, `_starts_with`, `_ends_with`, `_to_lower`,
  `_compare_to`, `_replace_first`, `_replace_range`, `_split_map_pieces`, `_string_equal`,
  `_string_juntar`), `_string_buffer_new`/`_write`, `_regexp_new`, `_print_list`/`_map`/`_set`,
  `_int_to_radix_string`, `_map_get_to_string`. P5 confere por busca que nenhum código emite cada
  um antes de apagar (algumas já não são emitidas, §1.7).
* **Ficam:** `dartforge_string_new` (JIT), `_string_concat`, `_string_equal` (só se ainda
  emitida; senão sai), `_string_juntar_tipado`, `_to_string_i64`/`_f64`/`_bool`/`_handle`
  (interpolação, `EN/lower/expressoes.rs:386,394,415`), `_identical` (caminho lento de
  `@df.identico`), `_equal`, e o resto (RTI, exceções, raízes, seletores, FFI, E/S).

### 3.8 Natives do SDK: o que muda de implementação

| Native / intrínseco | Hoje | Depois | Dono |
|---|---|---|---|
| `String_getLength`, `_StringBase.codeUnitAt` | `lower/textos.rs` com cache + `texto_dados`/`_len` | `@df.texto_len`/`@df.texto_unidade` com o teste de limites; fora da faixa, o native (lança) | P1 |
| `Internal_allocateOneByteString`/`TwoByte` | `nativos_strings.rs:142,150` | `@df.texto_alocar` em linha | P1 |
| `Internal_writeIntoOneByteString`/`TwoByte` | `nativos_strings.rs:157,169` (`get_mut`) | `@df.texto_gravar` em linha | P1 |
| `String_getHashCode` | `hash_de_texto` do slot | `@df.texto_hash` | P1 |
| `String_charAt`, `String_concat`, `StringBase_substringUnchecked`, `OneByteString_substringUnchecked`, `OneByteString_allocateFromOneByteList`, `TwoByteString_allocateFromTwoByteList`, `StringBase_createFromCodePoints`, `StringBase_joinReplaceAllResult`, `String_concatRange`, `String_toUpperCase`/`toLowerCase`, `DartForge_string_*` (7), `DartForge_sb_*` (5), `DartForge_int_toString`, `Double_toString`, `DartForge_double_bits`, `DartForge_regexp_*` (8), `DartForge_imprimir` | runtime sobre `Texto` | runtime sobre `TextoRef`/`novo_texto` | P1 |
| `ClassID_getID`, `ClassID.cidX` | `value_class`; constantes pela tabela | `@df.classe`; `layout::cid` | P0 (catálogo), P3 (`intrinsecos.rs`) |
| `List_getLength`, `GrowableList_getLength`, `_getCapacity` | runtime | `@df.lista_len` / comprimento do armazenamento | P3 (runtime), P4 (em linha, `tipados.rs`) |
| `GrowableList_setLength`, `_setData`, `_setIndexed`, `List_setIndexed`, `DartForge_lista_get` | runtime | em linha: gravação de campo sem barreira (comprimento, como a VM, `graph_intrinsifier.cc:607-626`), com barreira (`dados`, elemento geral) | P3 (runtime), P4 (em linha) |
| `List_allocate`, `List_slice`, `ImmutableList_from`, `GrowableList_allocate`, `DartForge_List_preencher`/`_copiar`, `DartForge_GrowableList_reservar`, `Internal_makeListFixedLength`/`makeFixedListUnmodifiable`, `DartForge_json_acrescentar`, `DartForge_hash_*` | runtime sobre `Elementos` | runtime sobre `listas.rs`; as duas `Internal_make*` devolvem objeto novo | P3 |
| `Closure_equals`, `Closure_computeHash`, `Function_apply`, `DartForge_record_*`, `Identical_comparison`, `Double_*`/`Integer_*`/`Mint_*`/`Smi_*`, `Double_parse`, `DartForge_double_*`, `DartForge_int_hashCode`, `Object_getHash` | runtime sobre `Value` | runtime sobre `caixas.rs`; `DartForge_record_*` em linha | P2 |
| `TypedDataBase_length`, `TypedDataView_typedData`/`_offsetInBytes`, `TypedData_Get*`/`Set*`, `DartForge_typed_*`, `_memMove*`, `setClampedRange`, SIMD | runtime sobre `Armazenamento` + `typed_cabecalho` | em linha por `@df.tipada_*`; runtime sobre `tipadas.rs` | P4 |
| `WeakReference_*`, `WeakProperty_*`, `Object_*` (menos hash), `Isolate_*`, `DartForge_porta_*` | runtime | runtime (API nova) | P5 |

O catálogo `EN/nativos.rs` (estado, `INTRINSECOS`, `RETORNO_REF`) é de P0, que aplica esta tabela.

---

## 4. Pacotes de trabalho

### 4.1 Regras

1. **Cada arquivo tem um dono** (matriz em §4.8). Ninguém edita arquivo de outro pacote; o que
   precisa passar de um arquivo a outro (natives que moram no arquivo errado) segue a tabela de
   mudanças (§4.9): o dono de origem apaga, o de destino recria com o mesmo símbolo.
2. **As interfaces são as de §3.** Mudança de assinatura é pedida ao dono e registrada aqui.
3. **A árvore compila o tempo todo.** P0a (§4.2) entrega o contrato com esqueletos (`todo!()`)
   de toda função de §3 e o emissor já delegando a métodos por pacote; cada pacote troca o
   esqueleto pela implementação e migra os seus arquivos com `cargo check` verde. A API velha
   (`Value`, `get`, `Texto` em slot…) convive até o **corte** (§4.8, passo final de P0), que a
   apaga; o que ainda a usar aparece como erro de compilação.
4. **Nada roda antes da integração.** Entre P0a e o corte o executável compila mas não é
   testado (a especificação admite a quebra). Teste de unidade de cada pacote que não dependa de
   outro é bem-vindo.
5. Comentários em português; sem `cargo fmt` em arquivo alheio; sem commit (o proprietário
   revisa e commita).

### 4.2 P0 — Fundação: layout, espaço, coletor, contrato do emissor

**Possui:** `RT/heap.rs`, `RT/layout.rs` (novo), `RT/espaco.rs` (novo), `RT/lib.rs`,
`crates/runtime/build.rs`, `RT/gc_raizes.rs`, `RT/alocador.rs`, `RT/hash.rs`,
`crates/runtime/tests/*.rs`, `crates/runtime/CONTRACT.md`; `EN/llvm/mod.rs`,
`EN/llvm/externs.rs`, `EN/llvm/raizes.rs`, `EN/llvm/seletores.rs`, `EN/hir.rs`, `EN/context.rs`,
`EN/sdk_modulo.rs`, `EN/nativos.rs`, `EN/lower/mod.rs`, `EN/lower/fn_builder.rs`,
`EN/lower/sdk_fonte.rs`, `EN/lower/externos.rs`, `EN/cache.rs`.

**Fornece:** §3.1, §3.2, §3.4, os helpers P0 de §3.5, §3.6, a numeração com cids fixos, os
ganchos do emissor.

**P0a — o contrato (≈ 1 dia; antes de todos):**

1. `layout.rs` completo (§3.1), com testes dos asserts, de `classe_de_tamanho` (monótona,
   `palavras_da_classe(classe_de_tamanho(w)) ≥ w`) e de `hash_de_texto` contra `Texto::hash_vm`.
2. Módulos novos registrados em `lib.rs`, `RUNTIME_MAIN` e `conferir_lista` (`build.rs:189-`):
   `layout`, `espaco`, `textos`, `caixas`, `listas`, `tipadas`. Os quatro últimos com as
   assinaturas de §3.3 e corpo `todo!("P<n>")`; os tipos de §3.3 (`TextoRef`, `ClosureRef`,
   `Elemento`, `ElementosRef`, `TipadaRef`) já definidos.
3. `heap.rs`: a API de §3.2 declarada ao lado da velha (corpo `todo!()` onde ainda não existe),
   `Valor`, os estáticos de `bool`.
4. Emissor: em `llvm/mod.rs`, os braços que dependem da representação passam a chamar métodos
   `emitir_*` de arquivos novos por pacote, com corpo `todo!()`:
   `llvm/textos_ir.rs` (P1: `Const(String/StringWtf8)`, `JuntarTextos`),
   `llvm/caixas_ir.rs` (P2: `Box`, `Unbox`, `AllocCell`, `CellGet`, `CellSet`, `AllocEnv`,
   `EnvGet`, `AllocClosure`, `AllocClosureTipada`, `TearOff`, `AllocRecord`),
   `llvm/listas_ir.rs` (P3: `AllocList`), `llvm/tipados_ir.rs` (P4: caixa e descaixa SIMD,
   hoje 1150-1169). Cada um exporta `pub const AJUDANTES: &str` (vazio), concatenado por
   `emit_runtime_decls`. Em `lower/sdk_fonte.rs` (`chamar_externo`, 686-794), depois de
   `texto_em_linha`: `lista_em_linha` (P3, `lower/listas.rs` novo), `tipada_em_linha` (P4,
   `lower/tipados.rs`), `caixa_em_linha` (P2, `lower/caixas.rs` novo), todas devolvendo
   `Option<Operand>` com `None` no esqueleto.
5. `externs.rs` com §3.6; `context.rs`/`sdk_modulo.rs` com a numeração de §2.4 (cids de
   `layout::cid::DO_SDK`, os demais a partir de 128); `nativos.rs` com a tabela §3.8.

**P0b — a implementação (≈ 8–10 dias, em paralelo com os demais):**

6. `espaco.rs`: `EspacoDeObjetos` indexado por classe (89), classes médias, regiões grandes com
   granularidade de 4 KiB (`VirtualAlloc`/`mmap`) e cache das regiões soltas por tamanho,
   `PERMANENTE`, `FORMA`, cartões, anexos (`anexos_nativos`), bytes externos.
7. `heap.rs`: marcação por formato (§2.8) com `PERMANENTE` conferido antes do mapa de marcas;
   lembrados com cartões; verificação; tabelas laterais sem slots; `permanentes`/`constantes`
   purgados; gatilhos com bytes externos; `Contexto` renumerado (§3.4); registro das faixas da
   imagem (`dartforge_registrar_imagem(inicio, fim)`, chamado por `@df.preparar_isolado` de cada
   imagem: executável e DLL do SDK) para validar estáticos no `--gc-stress`.
8. `llvm/mod.rs`: `@df.classe` sem slot; `@df.alocar` (substitui `emitir_alocacao_em_linha`,
   2622-2676, para qualquer `w ≤ 16`, constante ou não); `@df.barreira` com o teste do filho;
   `@df.barreira_elemento`; `@df.e_objeto`; `@df.subclasse` com os deslocamentos novos; saem
   `@df.env_ref`, `@df.caixa_int` e `@df.desencaixa_int` (vão a P2); o cache de literal no ponto de
   uso (578-605) só com `objetos_estaticos` falso; a opção `objetos_estaticos` (verdadeira no AOT
   e nos módulos do SDK; falsa nos módulos do programa no JIT, cuja memória é liberada, J02).
9. Reescrever os testes do módulo `heap` (4931-6216) para a API nova e acrescentar: um por
   formato; cartões (gravação velho → jovem em lista grande sobrevive à menor); anexo solto quando
   o dono morre; região grande devolvida; estático nunca marcado nem varrido; purga de
   `permanentes`; gatilho com bytes externos.
10. Remedir `LIMITE_JOVEM` e `CONTAGEM_JOVEM` com `bench/desempenho` (§2.8).
11. **Corte** (último passo, depois que P1–P5 migraram): apagar `enum Value` e tudo de §2.17;
    `cargo check` do workspace aponta o que sobrou.

**Pronto quando:** `cargo test -p dartforge-runtime` verde (o módulo `heap` inteiro e os testes
novos, também com `DARTFORGE_GC_VERIFICAR=1`); o workspace compila depois do corte; o IR de um
programa com string, lista, closure e `double` passa no verificador do LLVM.

### 4.3 P1 — Strings (fase B)

**Possui:** `RT/textos.rs` (novo), `RT/strings.rs`, `RT/nativos_strings.rs`, `RT/regexp.rs`;
`EN/llvm/textos_ir.rs` (novo), `EN/lower/textos.rs`; `sdk_nativo/core/string_patch.dart`,
`string_buffer_patch.dart`, `integers.dart`, `double.dart`, `regexp_patch.dart`,
`sdk_nativo/convert/convert_patch.dart`.

**Consome:** §3.1, §3.2 (`alocar`, `cabecalho`, `bytes`, `anexar`, `Valor`), `caixas.rs` (P2) para
os natives que devolvem número. **Fornece:** `textos.rs` (§3.3), os helpers P1 de §3.5,
`emitir_const_string` e `emitir_juntar_textos`, `texto_em_linha`.

1. Mover `Texto`, `TextoMut`, `IterUnidades` e `empurrar_ponto` (`RT/heap.rs:97-560`) para
   `textos.rs` (P0 deixa o `pub use`); `TextoRef` com os algoritmos; `Texto` delega à vista.
2. `impl Heap` de strings (§3.3) sobre `alocar` (`BRUTO`, cid 6/7, forma canônica), hash no
   `mapa`, `string_literal` internando em `literais`.
3. `strings.rs` e `nativos_strings.rs` sobre a vista: somem `alocar_texto` por slot, `texto_de`
   (cópia), `com_texto`, `texto_de_qualquer`, `padrao_de` por `Value`. Os externs do lowering por
   nome (§3.7) saem com o legado (P5 confirma que ninguém os emite).
4. `StringBuffer`: `_AcumuladorDeTexto` (cid 15, `ANEXO`, `Vec<u16>`) nos `DartForge_sb_*`.
5. `RegExp`: `_ProgramaDeRegExp` (cid 16, `ANEXO`) em `regexp.rs`; na sobreposição,
   `_RegExp._id` vira `final Object _programa` e os natives recebem o handle; sai `PROGRAMAS_RE`
   (hoje cresce sem limite, `RT/regexp.rs:1100-1102`).
6. Emissor: `llvm/textos_ir.rs` com os helpers, o literal estático (§2.11: `@df.s.<blake3>`
   `linkonce_odr`, comdat, seção da imagem, hash calculado com `layout::hash_de_texto`) e
   `JuntarTextos` sobre as vistas; `lower/textos.rs` sem os caches (121-188): `codeUnitAt` e
   `length` por `@df.texto_len`/`@df.texto_unidade` com o teste de limites e o native como
   caminho lento; `allocateOneByteString`/`TwoByteString` e `writeInto*` em linha;
   `String_getHashCode` por `@df.texto_hash`; `==` com os dois lados `String` estáticos por
   `@df.texto_igual`. O campo `caches_de_texto` do `FnBuilder` sai (P0, `lower/fn_builder.rs`).
7. Medir e decidir se `integers.dart` volta ao `_Smi.toString` da VM (agora `_allocate`/`_setAt`
   em linha, como `asm_intrinsifier_x64.cc:1754-1818`) e se o `==` volta ao da VM. Registrar.
8. Receber os natives de §4.9.

**Pronto quando:** compila contra a API; no fim, `corpus/nativo` 49, 56, 61, 75 e 90–94 e os
programas de string de `corpus/js` (pares substitutos inclusive) iguais à VM;
`bench/desempenho/json.dart` e `textos.dart` com a saída da VM.

### 4.4 P2 — Caixas, células, contextos, closures, records (fase C)

**Possui:** `RT/caixas.rs` (novo), `RT/nucleo.rs`, `RT/closures.rs`, `RT/despacho.rs`,
`RT/nativos_numeros.rs`; `EN/llvm/caixas_ir.rs` (novo), `EN/lower/caixas.rs` (novo),
`EN/lower/closures.rs`, `EN/lower/locais.rs`, `EN/lower/captura.rs`, `EN/lower/extensoes.rs`,
`EN/lower/async_sm.rs`, `EN/lower/registros.rs`, `EN/lower/tipos_de_extensao.rs`,
`EN/lower/funcoes_diretas.rs`; `sdk_nativo/core/function.dart`.

**Consome:** §3.1, §3.2. **Fornece:** `caixas.rs` (§3.3), os helpers P2 de §3.5, os braços de
caixa/célula/contexto/closure/record, `caixa_em_linha`.

1. `caixas.rs`: `_Mint`/`_Double` (`BRUTO`, w = 1), `bool` estático, `valor`/`como_ref`,
   `identico` (§2.10), célula, contexto, closure, record (`REFS`), `tearoff` canônico.
2. `nucleo.rs`: `dartforge_box_*`/`unbox_*` (só caminho lento), `dartforge_identical`,
   `dartforge_equal`, `dartforge_value_class` trivial (cid do cabeçalho), `dartforge_record_novo`,
   `escalar_de_ref`, `dartforge_object_new` por palavras (`alocar_instancia` +
   `reabastecer_tlab(palavras_de_instancia(n))`); sai a ABI de pares `tagged`/`untag`
   (401-425) onde os valores passam a ir como palavras.
3. `closures.rs`, `despacho.rs`, `nativos_numeros.rs` sobre `caixas.rs`; `Function.apply`
   (269-306).
4. Emissor: `llvm/caixas_ir.rs` (helpers e braços: `AllocEnv`/`AllocCell`/`AllocClosure*` em
   linha por `@df.alocar` e gravação de campo com o bit do mapa; `EnvGet`/`CellGet` lidos no tipo
   gravado; `AllocRecord` com `Ref`s); `lower/closures.rs` lê `abi` (`h+38`), `tipado` (`h+30`) e o
   contexto (`h+22`) do bloco, sem `closure_cabecalho` (794-822); `lower/locais.rs`,
   `captura.rs`, `extensoes.rs`, `async_sm.rs`, `funcoes_diretas.rs`: `EnvGet`/`CellGet` sempre na
   representação com que a captura foi gravada (a do tipo da variável); `lower/registros.rs`:
   record posicional como `REFS`.
5. `function.dart`: declarar os campos do `_Closure` na ordem do layout
   (`final int _codigo; final Object? _contexto; final int _tipado; final int _abi;`) no lugar dos
   da VM, que o runtime nunca usou, para `hashCode`/`==` poderem ser Dart; ou manter os natives
   (P2 decide e registra).
6. Receber os natives de §4.9.

**Pronto quando:** compila; no fim, `corpus/nativo` 25, 28, 41, 50 (os dois), 52, 54, 60, 63, 64,
68, 80 e 95 iguais à VM;
`bench/desempenho/objetos_*.dart` e `chamadas.dart` com a saída da VM.

### 4.5 P3 — Listas, mapas, conjuntos (fase D, núcleo)

**Possui:** `RT/listas.rs` (novo), `RT/nativos_listas.rs`, `RT/colecoes.rs`, `RT/nativos_hash.rs`;
`EN/llvm/listas_ir.rs` (novo), `EN/lower/listas.rs` (novo), `EN/lower/literais.rs`,
`EN/lower/intrinsecos.rs`, `EN/lower/constantes.rs`; `sdk_nativo/core/array.dart`,
`growable_array.dart`, `sdk_nativo/collection/compact_hash.dart`, `collection/list.dart`,
`sdk_nativo/internal/sort.dart`.

**Consome:** §3.1, §3.2, `caixas.rs` (P2: `como_ref`, `valor`), `textos.rs` (P1: `hash_de_texto`,
`textos_iguais`), `tipadas.rs` (P4: o `_index` do `_Map`). **Fornece:** `listas.rs` (§3.3), os
helpers P3 de §3.5, o braço `AllocList`, `lista_em_linha`.

1. `listas.rs`: `_List`/`_ImmutableList` gerais (`REFS`) e compactas (`BRUTO`+`ELEMENTO`),
   `_GrowableList` (`INSTANCIA`: comprimento e dados), crescimento `(capacidade * 2) | 3`
   (`growable_array.dart:385` da VM), conversão de forma sem alocação entre a mudança dos dados e
   a do `flags` (§2.16), cópias com barreira em bloco, cartões nas grandes.
2. `nativos_listas.rs`: os natives de §1.8 sobre `listas.rs`; `Internal_makeListFixedLength` e
   `Internal_makeFixedListUnmodifiable` devolvem objeto novo (§2.16); os natives de outros
   pacotes saem (§4.9).
3. `colecoes.rs`: `dartforge_collection_mark_unmodifiable` passa a devolver o `_ImmutableList`
   novo (e `lower/constantes.rs:486` usa o valor devolvido); o resto é legado e sai (P5 confirma).
4. `nativos_hash.rs`: a sonda sobre o `_data` (`REFS`: `Smi`, `_Mint`, strings) e o `_index`
   (`bytes_da_tipada`).
5. Emissor: `llvm/listas_ir.rs`; `lower/listas.rs` (os natives de `_List`/`_GrowableList` em
   linha: comprimento, capacidade, `_setLength` sem barreira, `_setData` e `_setIndexed` com
   barreira, `DartForge_lista_get`); `lower/literais.rs` (`AllocList` com a forma do `E` e as
   palavras; a tabela de bytes de `lista_de_tabela`; apaga os ramos sem SDK, 53-121); `lower/intrinsecos.rs` (`ClassID.cidX` por
   `layout::cid`; o `add` escalar grava direto quando cabe e chama `dartforge_lista_acrescentar`
   quando cresce); `lower/constantes.rs`.
6. Receber e entregar os natives de §4.9.

**Pronto quando:** compila; no fim, `corpus/nativo` 10, 21, 22, 26, 27, 30, 37, 40, 46 (os dois),
47 (os dois), 85, 87, 96 e 97 iguais à VM;
`bench/desempenho/colecoes.dart` e `objetos_em_colecoes.dart` com a saída da VM.

### 4.6 P4 — Listas tipadas, SIMD, FFI, E/S (fase D, dados)

**Possui:** `RT/tipadas.rs` (novo), `RT/typed_data.rs`, `RT/simd.rs`, `RT/ffi.rs`,
`RT/ffi_callbacks.rs`, `RT/io_arquivos.rs`, `RT/io_diretorios.rs`, `RT/io_eventos.rs`,
`RT/io_observador.rs`, `RT/io_plataforma.rs`, `RT/io_processos.rs`, `RT/io_servico.rs`,
`RT/io_soquetes.rs`, `RT/io_soquetes_unix.rs`, `RT/io_windows_eventos.rs`,
`RT/io_windows_processos.rs`, `RT/io_windows_soquetes.rs`, `RT/tls.rs`, `RT/tls_formatos.rs`,
`RT/zlib.rs`; `EN/llvm/tipados_ir.rs` (novo), `EN/llvm/simd.rs`, `EN/lower/tipados.rs`,
`EN/lower/comandos.rs`, `EN/lower/simd.rs`, `EN/lower/ffi.rs`; `sdk_nativo/ffi/*`.

**Consome:** §3.1, §3.2, os helpers de lista de P3 (acesso em linha a `List<E>` em
`tipados.rs`), `caixas.rs` (P2). **Fornece:** `tipadas.rs` (§3.3), os helpers P4 de §3.5, os
braços SIMD, `tipada_em_linha`, o acesso em linha a listas tipadas e a `List<E>`.

1. `tipadas.rs`: internas (`BRUTO`, `dados` apontando para `b+32`), externas (`EXTERNO`), visões
   (`INSTANCIA`, `dados` calculado), SIMD.
2. `typed_data.rs`, `simd.rs`: natives sobre `TipadaRef` (`resolver`, `bytes_de`, `bytes_de_mut`,
   `simd_bytes`).
3. `ffi.rs`, `ffi_callbacks.rs`: `asTypedList` → `tipada_externa`; `com_memoria` e compostos por
   `dados`; a lista de argumentos dos callbacks por `listas.rs`.
4. `io_*.rs`, `tls.rs`, `zlib.rs`: `dart_bytes`, `dart_lista_fixa`, `dart_int`, `dart_bool`,
   `bytes_da_lista_tipada` sobre a API nova; `tls.rs` sem o `mem::take` (§2.14).
5. Emissor: `llvm/tipados_ir.rs` e `llvm/simd.rs` (caixa por `@df.simd_caixa`, descaixa por
   carga de `h+14`); `lower/tipados.rs`: lista tipada por `@df.tipada_len`/`_dados` (saem
   `typed_cabecalho` e o cache por ponto, 466-568), `List<E>` por `@df.lista_*` com a forma no
   `flags` do armazenamento, gravação direta quando o cid é 8 ou 10 e a forma casa, barreira de
   elemento na lista geral; `lower/comandos.rs` (laços versionados, 322-344);
   `lower/simd.rs`, `lower/ffi.rs`.
6. Coordenação: `lower/comandos.rs` e `lower/tipados.rs` são hoje da frente de vetorização; P4
   começa depois que ela fechar ou combina a ordem com ela.

**Pronto quando:** compila; no fim, `corpus/nativo` 01–06, 08, 09, 11–13, 15–20, 24, 31, 32, 34,
38, 39, 55, 62, 67, 69, 77, 78 e 98 iguais à VM;
`bench/simd` e `bench/desempenho/tipados.dart` com a saída da VM;
`crates/cli/tests/io_regressao.rs` verde.

### 4.7 P5 — Transversais e remoção do legado

**Possui:** `RT/portas.rs`, `RT/isolados.rs`, `RT/ffi_api_nativa.rs`, `RT/tipos.rs`,
`RT/seletores.rs`, `RT/excecoes.rs`, `RT/saida.rs`, `RT/compat_jit.rs`, `RT/eventos.rs`,
`RT/finalizadores.rs`, `RT/nativos_sistema.rs`, `RT/nativos_desenvolvedor.rs`;
`EN/lower/sdk_por_nome.rs` (apaga), `EN/lower/expressoes.rs`, `atribuicao.rs`, `padroes.rs`,
`membros.rs`, `chamadas.rs`, `verificador.rs`, `operadores.rs`, `enums.rs`, `nsm.rs`,
`erros_do_runtime.rs`, `despacho.rs`, `heranca.rs`, `rti.rs`, `cascata.rs`,
`const_primitiva.rs`, `entrada_tipada.rs`, `EN/otimizar/*`, `EN/llvm/testes.rs`,
`EN/tests/*`, `crates/jit/**`, `crates/cli/tests/*`; `sdk_nativo/isolate/*`,
`sdk_nativo/core/identical_patch.dart`, `finalizer_patch.dart`, `sdk_nativo/internal/print_patch.dart`,
`sdk_nativo/io/*`; `corpus/nativo/90…99`; `docs/*`.

**Consome:** tudo. **Fornece:** isolados, RTI, saída, erros e seletores sobre a API nova; o
emissor sem o legado; os testes e a documentação.

1. `portas.rs`: `NoG::Bloco` genérico e `ValG::Estatico` (§2.12); `isolados.rs`
   (`tratar_mensagem_de_controle` por `listas.rs`/`bool_de`, `relatar_erro_nao_tratado` por
   `texto`); `ffi_api_nativa.rs`.
2. `tipos.rs`: `chave_do_valor` e `tipo_do_valor` pelo cid (§2.15); `tipo_lista_*`;
   `ajustar_forma_da_lista` → `lista_ajustar_forma`; `set_metadado` uniforme.
3. `seletores.rs`: saem `cid_do_valor_do_runtime`, `cid_do_runtime`, `CIDS_DO_RUNTIME`;
   `dartforge_registrar_cids` confere a ABI; `aplicar_migracao_pendente` recusa cid < 128.
4. `excecoes.rs`, `saida.rs` (impressão e `toString` de tudo pelo cid), `compat_jit.rs`,
   `eventos.rs`, `finalizadores.rs`, `nativos_sistema.rs`, `nativos_desenvolvedor.rs`.
5. Receber os natives de §4.9 (`Object_*`, `WeakReference_*`, `WeakProperty_*`).
6. Emissor: apagar `lower/sdk_por_nome.rs`, `sdk_da_fonte_pedido`/`DARTFORGE_SDK_DA_FONTE`
   (`EN/sdk_modulo.rs:122-128`) e os ramos `!sdk_da_fonte` (`expressoes.rs:1161-1311`,
   `atribuicao.rs:703,721,896`, `padroes.rs:474,491`, `membros.rs:525,534`, `chamadas.rs:50,139,
   223,592,618`; os de `literais.rs:53-121` são de P3); com P0, `AllocMap`/`AllocSet` da HIR; os externs
   legados de §3.7 depois de conferir por busca que ninguém os emite. `operadores.rs`:
   `identical` por `@df.identico`, `==` de strings por `@df.texto_igual`. `verificador.rs`:
   recusa `EnvGet`/`CellGet` em tipo diferente do gravado e as instruções que sumiram.
   `otimizar/*`: efeitos das externs novas; `escape.rs` continua só para objetos do programa.
7. Testes: `EN/tests/contrato.rs:36` e `crates/jit/tests/sessao_persistente.rs:148` sem o modo
   legado; `EN/llvm/testes.rs`; os programas 90–99 (§5.3) com as saídas da VM.
8. Documentação: `NATIVO.md` (a representação), `NATIVO-PLANO.md` (seção nova com os números),
   `PLANO-TAMANHO-DESEMPENHO.md` (fases B–D), `JIT.md` (sem estáticos nos módulos do programa),
   `crates/runtime/CONTRACT.md` (com P0).

**Pronto quando:** §5 inteiro verde.

### 4.8 Matriz arquivo → pacote e ordem

| Pacote | Runtime | Emissor | SDK nativo |
|---|---|---|---|
| P0 | heap, layout\*, espaco\*, lib, build.rs, gc_raizes, alocador, hash, tests/, CONTRACT.md | llvm/mod, llvm/externs, llvm/raizes, llvm/seletores, hir, context, sdk_modulo, nativos, lower/mod, lower/fn_builder, lower/sdk_fonte, lower/externos, cache | — |
| P1 | textos\*, strings, nativos_strings, regexp | llvm/textos_ir\*, lower/textos | core/string_patch, string_buffer_patch, integers, double, regexp_patch; convert/convert_patch |
| P2 | caixas\*, nucleo, closures, despacho, nativos_numeros | llvm/caixas_ir\*, lower/caixas\*, lower/closures, locais, captura, extensoes, async_sm, registros, tipos_de_extensao, funcoes_diretas | core/function |
| P3 | listas\*, nativos_listas, colecoes, nativos_hash | llvm/listas_ir\*, lower/listas\*, lower/literais, intrinsecos, constantes | core/array, growable_array; collection/compact_hash, list; internal/sort |
| P4 | tipadas\*, typed_data, simd, ffi, ffi_callbacks, io_\* (12), tls, tls_formatos, zlib | llvm/tipados_ir\*, llvm/simd, lower/tipados, comandos, simd, ffi | ffi/\* |
| P5 | portas, isolados, ffi_api_nativa, tipos, seletores, excecoes, saida, compat_jit, eventos, finalizadores, nativos_sistema, nativos_desenvolvedor | lower/sdk_por_nome (apaga), expressoes, atribuicao, padroes, membros, chamadas, verificador, operadores, enums, nsm, erros_do_runtime, despacho, heranca, rti, cascata, const_primitiva, entrada_tipada, otimizar/\*, llvm/testes, tests/\*; crates/jit, crates/cli/tests | isolate/\*, core/identical_patch, finalizer_patch, internal/print_patch, io/\* |

\* arquivo novo. Os demais arquivos do emissor (`alvo`, `apagamento`, `bin/*`, `cache_objeto`,
`driver`, `fonte`, `gerador`, `ligador*`, `llvm/abi_c`, `llvm/depuracao`, `resumo`) não mudam;
se mudarem, são de P0.

Ordem:

1. **P0a** (≈ 1 dia). Todos param até ele entrar.
2. **P0b, P1, P2, P3, P4, P5 em paralelo.** Esforço estimado: P0b 8–10 dias, P1 5–7, P2 5–7,
   P3 6–8, P4 5–7 (depois da frente de vetorização), P5 8–10.
3. **Corte** (P0): apaga a API velha; os pacotes corrigem o que o compilador apontar nos seus
   arquivos.
4. **Integração** (todos, 5–10 dias): §5 na ordem, cada falha com o dono do arquivo.

### 4.9 Natives que mudam de arquivo

| Native (linha atual em `RT/nativos_listas.rs`) | Para | Pacote |
|---|---|---|
| `DartForge_string_iguais` 505, `DartForge_string_igual_a` 514, `String_concatRange` 541, `OneByteString_allocateFromOneByteList` 763, `TwoByteString_allocateFromTwoByteList` 770, `StringBase_createFromCodePoints` 778, `StringBase_joinReplaceAllResult` 797, `DartForge_imprimir` 498 | `nativos_strings.rs` | P1 |
| `ClassID_getID` 479, `Identical_comparison` 485, `DartForge_int_hashCode` 563, `DartForge_double_*` 661-723, `Closure_equals` 829, `Double_parse` 854, `DartForge_math_*` 907-950, `DartForge_record_*` 954-986, `Closure_computeHash` 1060 | `nucleo.rs` ou `nativos_numeros.rs` | P2 |
| `Object_*` 491-619 (menos o hash de identidade, que usa `hash_de_identidade`), `WeakReference_*`/`WeakProperty_*` 1027-1054 | `nativos_sistema.rs` | P5 |

P3 apaga as funções de `nativos_listas.rs`; o destino as recria com o mesmo símbolo (o `build.rs`
recusa símbolo duplicado, `crates/runtime/build.rs` "define algum símbolo duas vezes").

### 4.10 Registro de mudanças do contrato (P0a, 2026-09-30)

O que a P0a entregou diferente do texto acima, ou além dele (regra 2 de §4.1):

1. **Nomes velhos com sufixo `_velho`.** Sete métodos da API velha tinham o nome que §3.3 dá a
   um método novo com outra assinatura, e um tipo não pode ter os dois: `Heap::texto`,
   `hash_de_texto`, `string_literal` (P1) e `caixa_int`, `caixa_bool`, `como_ref`, `tearoff`
   (P2) viraram `texto_velho`, `hash_de_texto_velho`, `string_literal_velho`, `caixa_int_velho`,
   `caixa_bool_velho`, `como_ref_velho`, `tearoff_velho`, com as chamadas dos fragmentos
   trocadas (mecânico, 54 pontos). Somem no corte. Os nomes sem sufixo são os de `textos.rs` e
   `caixas.rs`.
2. **`e_objeto`** mora em `layout` e é `h & (7 | i64::MIN) == 2` (o texto de §2.2 diz `& 3`; os
   dois coincidem em todo handle válido, porque os blocos são alinhados a 8 e os handles de slot
   são múltiplos de 4). `heap.rs` reexporta de `layout` `Cabecalho`, `Ref`, `e_objeto`, `smi`,
   `palavras_do_mapa`, `capacidade`, `PAGINA`, `CABECA_DA_PAGINA`, `DESLOCAMENTO_DO_HANDLE`,
   `MAIOR_CLASSE`, `TLAB_N`, `CAMPOS_EM_LINHA`; `LIVRE`/`JOVEM`/`VELHO`/`LEMBRADO`/`FORA` são
   apelidos das de `layout`, e `MARCADO` (2) fica enquanto o coletor de hoje o usar.
3. **`novo_objeto`** continua com `class_id: i64` até o corte (os chamadores passam `i64`);
   `hash_de_identidade` passou a `&self` (compatível). `alocar_instancia`, `cabecalho`,
   `classe` (objeto, null e `Smi`), `e_objeto_vivo` e `lembrar` já funcionam sobre o espaço de
   hoje; o resto de §3.2 é `todo!("P0b")`.
4. **`layout` a mais que §3.1:** `cid::INTERNA` (a biblioteca `""` das classes internas do
   runtime em `DO_SDK`; as do SDK usam a URI, `dart:core`, a chave da numeração estável),
   `cid::TIPOS_DE_ELEMENTO` e os predicados por faixa de §2.4 (`cid::e_texto`, `e_lista_fixa`,
   `e_lista`, `e_tipada_interna`, `e_tipada`, `e_visao_imutavel`, `e_simd`),
   `palavras_de_cartoes`, `bytes_do_bloco` e `contexto::tlab_cursor(w)`/`tlab_fim(w)`.
5. **Emissor: os corpos não são `todo!()`.** Os arquivos `llvm/textos_ir.rs`, `caixas_ir.rs`,
   `listas_ir.rs` e `tipados_ir.rs` receberam o código de hoje dos braços, movido sem mudança: o IR
   gerado é o mesmo até cada pacote trocar o seu. Os métodos: `emitir_const_string`,
   `emitir_juntar_textos`, `buffer_de_juntar_textos`, `emitir_globais_de_texto` (gancho no fim do
   módulo para os literais estáticos) (P1); `emitir_caixa`, `emitir_descaixa`, `emitir_alloc_cell`,
   `emitir_cell_get`, `emitir_cell_set`, `emitir_alloc_env`, `buffer_de_ambiente`,
   `emitir_env_get`, `emitir_alloc_closure`, `emitir_alloc_closure_tipada`, `emitir_tearoff`,
   `emitir_alloc_record`, `buffer_de_record` (P2); `emitir_alloc_list`, `buffer_de_lista` (P3);
   `emitir_caixa_simd`, `emitir_descaixa_simd` (P4). Os `buffer_*` são os `alloca` do bloco de
   entrada (`emit_buffers_de_closure`), que dependem da ABI de cada braço. Cada arquivo define um
   `Estado*` (`#[derive(Default)]`) guardado no `LlvmEmitter` (`self.textos`, `self.caixas`,
   `self.listas`, `self.tipados`), para o pacote ter estado por módulo sem editar `llvm/mod.rs`.
   O `LlvmEmitter` ganhou `objetos_estaticos` e `com_objetos_estaticos(bool)` (o driver liga na
   P0b). Os ganchos do lowering: `lower/listas.rs::lista_em_linha`,
   `lower/tipados.rs::tipada_em_linha`, `lower/caixas.rs::caixa_em_linha`, com a assinatura de
   `texto_em_linha` (`membro`, `native`, `this`, `args`).
6. **Externs novas com esqueleto no runtime.** O teste `cada_extern_do_emissor_existe_no_runtime`
   (`crates/jit/tests/execucao.rs`) exige cada extern declarada no runtime; os esqueletos
   (`todo!`) estão nos arquivos dos donos: `dartforge_alocar` (`gc_raizes.rs`, P0),
   `dartforge_texto_novo`/`_texto_hash`/`_texto_iguais` (`nativos_strings.rs`, P1),
   `dartforge_lista_nova`/`_lista_acrescentar` (`nativos_listas.rs`, P3),
   `dartforge_record_novo` (`nucleo.rs`, P2). `@dartforge_falso`/`@dartforge_verdadeiro` não são
   funções: ficam em `externs::GLOBAIS`, impressas no prelúdio.
7. **`ClassID_getID`** passou a `Embutido` no catálogo: `CallRuntime dartforge_value_class`, que o
   emissor já troca por `@df.classe`. O native do runtime fica até a P2 movê-lo (§4.9).
   `nativos::no_espaco_unificado(nome) -> Option<(Pacote, bool)>` codifica a tabela §3.8 (o
   pacote e se vira em linha).
8. **Numeração:** as classes do programa também começam em 128 quando não há SDK compilado.
   `sdk_modulo::cids_do_runtime` não mudou (a tabela de 32 posições ainda alimenta o runtime
   velho; com a numeração nova ela já dá os cids fixos).
9. **Problemas abertos para a P0b:** (a) os estáticos de `bool` existem em cada cópia do runtime
   (a `staticlib` do executável e a da DLL do SDK); a identidade de `true` exige que o código
   da DLL use os do executável (ou o contrário); (b) o JIT publica só funções
   (`simbolos::tabela`): os dados `dartforge_falso`/`_verdadeiro` precisam entrar na tabela;
   (c) a seção da imagem (`.dfimg`) dos estáticos.

**P2 (2026-09-30):**

10. **Tear-off canônico:** a P0 acrescentou `Heap::tearoff_registrado(codigo) -> Option<Ref>` e
    `Heap::registrar_tearoff(codigo, h)` (as tabelas `tearoffs`/`codigo_do_tearoff`/`permanentes`
    são privadas); `caixas.rs::tearoff` usa os dois.
11. **Leitura de `_Closure` e `_Record` no lowering:** `lower/closures.rs`, `lower/registros.rs`
    e `lower/caixas.rs` leem os campos pela `CallRuntime dartforge_object_get(h, i)` com `i`
    constante, que o emissor expande em linha como a palavra `b+16+8i` (zeros para null/`Smi`).
    A P0 mantém essa expansão sem conferir `FORMA` (vale também para a `_Record` `REFS`: `i = 0`
    é o número de campos, `i = k` o campo `k`). A chamada tipada confere `@df.classe == 11` antes
    de ler a ABI (só o bloco de `_Closure` tem os quatro campos).
12. **`AllocEnv` vazio é null** (sem alocar): quem não captura nada nunca lê o contexto. Quem lê
    um contexto no runtime (`dartforge_encaminhar_nsm`, P5) usa `Heap::captura(h, i)` só para
    `i` menor que o número de valores gravados.
13. **Record posicional:** `registros.rs::lower_registro_posicional(ast, positional)` encaixota
    os elementos em HIR (`Box`) e emite `AllocRecord` só com `Ref` (tag 3); pedido à P5 que o
    braço `ExprKind::Record` posicional de `expressoes.rs` o chame. Enquanto um elemento chegar
    sem caixa, o emissor usa o caminho lento `dartforge_record_new` (pares), que encaixota no
    runtime com as raízes certas: a extern fica declarada.
14. **Ajudantes da P2 no prelúdio:** `@df.caixa_int`, `@df.desencaixa_int`, `@df.caixa_double`,
    `@df.desencaixa_double`, `@df.caixa_bool`, `@df.desencaixa_bool`, `@df.identico`
    (`caixas_ir::AJUDANTES`, com `EFEITOS_DOS_AJUDANTES`). Usam `@df.alocar(cabeçalho, w)` e,
    em `CellSet` de `Ref`, `@df.barreira(o, v)` (P0). `@df.env_ref` não existe mais: o `EnvGet`
    lê a palavra `h+14+8i` na representação gravada.
15. **`caixa_em_linha`:** `Identical_comparison` por `CallRuntime df.identico` (I1),
    `DartForge_int_hashCode` (o próprio `i64`) e `DartForge_double_bits` (`Bitcast`). Os
    `DartForge_record_*` ficam no runtime: o receptor pode ser um record com forma (objeto do
    programa), e o native confere a classe.
16. **`sdk_nativo/core/function.dart`:** os campos do `_Closure` da VM saem; entram
    `int _codigo`, `Object? _contexto`, `int _tipado`, `int _abi` na ordem do bloco (só para a
    classe descrever o layout). `==` e `hashCode` continuam natives (`Closure_equals`,
    `Closure_computeHash`, em `nucleo.rs`): precisam do receptor guardado no `_Contexto`.
17. **`dartforge_value_class`** devolve o cid do layout (1 para null, 2 para `Smi`), não mais
    os negativos de antes (−12, −9…); quem os comparava no runtime (P5) passa ao cid.
18. **As funções de `Heap` da P2 que alocam enraízam elas mesmas os `Ref` recebidos**
    (`nova_celula`, `novo_contexto`, `nova_closure`, `novo_record`): quem chama só enraíza o
    resultado.
19. **Natives movidos (regra atômica de §4.9):** os de §4.9 da P2 e também `Object_getHash`
    (para `nucleo.rs`: `int` → o valor, `bool` → 1231/1237, null → 2011, `String` → o hash do
    conteúdo, o resto → `hash_de_identidade`) e `DartForge_verdadeiro` (`has63BitSmis`, para
    `nativos_numeros.rs`). O teste `testes_modulo_double` foi junto.
20. **`EXCEPTION` em `finalizar_programa`** (`nucleo.rs`) ainda lê o par `(bits, tag)` por
    `untag`: muda no corte, junto com o tipo da `EXCEPTION` (P5: `TaggedValue` → `Valor`).
    `tagged`/`untag`/`valor_como_ref` ficam em `nucleo.rs` até lá (os usam `excecoes.rs` e
    `colecoes.rs`).

19. **P4 — ajudantes do prelúdio** (`llvm/tipados_ir.rs`, com `EFEITOS_DOS_AJUDANTES`, todos
    sem alocar nem lançar, menos `@df.simd_caixa`): `@df.tipada_len`, `@df.tipada_dados`
    (devolve `i64`, não `ptr`: o lowering soma e indexa em HIR), `@df.tipada_cid`,
    `@df.tipada_len_gravavel(t, cid_imutável)` (0 na visão não modificável), `@df.tipada_bytes`,
    `@df.tipada_base`, `@df.tipada_deslocamento` (cargas com `!invariant.load`: nada disso muda
    enquanto a lista vive); `@df.nucleo_len` (0 se o cid não é 8–10), `@df.nucleo_armazenamento`,
    `@df.nucleo_forma` (`flags & 0x66` do armazenamento), `@df.nucleo_len_gravavel(l, forma)`
    (cid 8 ou 10 e forma igual), `@df.palavra_ref(e, i)` (carga de um `Ref` que o emissor
    enraíza, porque o resultado da `CallRuntime` é `Ref`); `@df.simd_caixa(<2 x i64>, cid)` por
    `@df.alocar`. Os `@df.nucleo_*` leem o bloco pelos deslocamentos do contrato em vez de
    chamar os `@df.lista_*` da P3 (sem dependência de ordem; mesma semântica). A descaixa SIMD
    lê `h+14` sem `!invariant.load` (a caixa pode ter sido gravada na mesma função).
20. **P4 — forma no lowering:** `lower/tipados.rs::codigo_da_forma` passa a devolver o `flags`
    do armazenamento (`REFS` 0x04, `BRUTO|ELEMENTO_*` 0x22/0x42/0x62), e `ListaFixa::forma`
    (N13) guarda esse código; `ListaFixa::dados` é o endereço do elemento 0 (`a+22`). Ganchos
    novos `pub(super)` para quem lê listas do núcleo (o for-in de `sdk_fonte.rs`):
    `comprimento_do_nucleo`, `armazenamento_da_lista`, `comprimento_gravavel_do_nucleo`,
    `ler_elemento_da_lista` (mesma assinatura). Gravação direta de `Ref` em lista geral (com
    `@df.barreira_elemento`) não foi feita: só escalares em forma compacta são gravados em linha,
    como antes.
21. **P4 — runtime:** `typed_data.rs::resolver` devolve `Option<TipadaRef>`; `bytes_de`/
    `bytes_de_mut` saem (usar `Heap::tipada`/`bytes_da_tipada[_mut]`). `TIPO_*` e
    `tamanho_do_elemento` ficam, agora apelidos de `tipadas::tipo`/`tipadas::tamanho_do_elemento`
    (há também `tipadas::tipo_do_cid` e `cid_da_visao`). `TipadaRef::fatia`/`fatia_mut`
    (`unsafe`) dão os bytes sem o empréstimo do heap (TLS, FFI). Auxiliares de fragmento em
    `io_arquivos.rs`: `dart_lista_fixa(&[Ref])` (`nova_lista(LIST, n, Geral)` + `gravar_refs`),
    `bytes_de_lista_de_valores(&Heap, h)`, `dart_tipada_de_bytes(tipo, &[u8])`. Saem do runtime
    `dartforge_typed_cabecalho`, `_typed_cabecalho_na_falha` e `dartforge_simd_caixa` (o código
    gerado não os emite mais); `dartforge_typed_len`/`_typed_ptr` ficam (caminho do
    `fillRange`), sem emissão. **Pedidos:** à P0, tirar essas três declarações de `externs.rs` e
    o caso especial `dartforge_typed_cabecalho`/`cabecalhos_invariantes` de `llvm/mod.rs`, e
    consultar `tipados_ir::EFEITOS_DOS_AJUDANTES` em `efeitos_de`; à P3, trocar `resolver`/
    `bytes_de` em `nativos_listas.rs` (`codigos_da_lista`); à P5, tirar `dartforge_typed_len`/
    `_ptr`/`_cabecalho` de `otimizar/simplificar.rs` e tratar os `df.*` sem efeito como puros.
22. **P4 — `tipada_em_linha`:** em linha `TypedDataBase_length`, `TypedDataView_offsetInBytes`,
    `TypedDataView_typedData`, o `[]` reconhecido das listas numéricas e visões e
    `_TypedList._getX`/`_setX` numéricos, com o runtime como caminho lento; SIMD, `_memMove*` e
    `setClampedRange` ficam no runtime. Os natives de `_List`/`_GrowableList` (§3.8, linha
    "P4 (em linha)") ficam com a P3 (`lista_em_linha`, §4.5 passo 5).

**P3 (2026-09-30):**

23. **`listas.rs` além de §3.3:** `Elemento::{flags, de_flags, codigo, do_codigo}` (o código de
    ABI é a ordem das variantes: 0 geral, 1 `int`, 2 `double`, 3 `bool`), `ElementosRef::{len,
    is_empty, get}`, `Heap::lista_de_refs(cid, &[Ref])` (lista fixa geral recém-alocada) e
    `Heap::lista_preencher(h, Valor)` (o `List.filled`: o mesmo objeto em toda posição).
    `lista_dados(h)` devolve o próprio `h` numa `_List`/`_ImmutableList` (como
    `@df.lista_armazenamento`). A `_List` compacta reserva as palavras de cartões como a geral
    (descompactar no lugar precisa delas), e a grande nasce com `CARTOES` em qualquer forma.
    Todo método que aloca enraíza o que segura entre duas alocações (inclusive o `v` de
    `lista_push` durante o crescimento); o resultado é de quem chama.
24. **Null numa lista compacta.** `lista_ajustar_forma(h, compacta)` aceita null dentro do
    comprimento (vira 0/0.0/false): é chamado depois de o `E` escalar não anulável ser gravado, e
    o null só existe antes de o SDK preencher (`List<int>.filled`, `_GrowableList<int>(n)`).
    `lista_set` de null numa compacta com metadado ≠ 0 grava zero (o `length =` que encolhe
    limpa as posições cortadas por `_setIndexed(i, null)`); sem metadado, descompacta. Conta com
    a P5: a forma compacta só para `E` exatamente `int`/`double`/`bool` não anulável, e todo
    `rti_definir` de lista chamando `lista_ajustar_forma` (com `Geral` para qualquer outro `E`).
    Numa `_GrowableList` de armazenamento vazio, ajustar a forma aloca um armazenamento vazio
    próprio (o `_emptyList` do SDK é compartilhado); ajustar pode alocar (descompactar), então
    quem chama enraíza `h`.
25. **`dartforge_lista_nova(palavras, n, cid, forma)`:** `forma` de 0 a 3 é o código de
    `Elemento`; **outro valor é o endereço de `n` bytes de tags** (1 `int`, 2 `bool`, 3 `Ref`,
    4 bits de `double`) do literal misto, que sai geral com as caixas feitas no runtime (uma
    caixa feita no código gerado antes da lista ficaria sem raiz na alocação seguinte). O
    `AllocList` passa `cid` 10 (`_GrowableList` de capacidade `n`) e escolhe a forma pelos tipos
    dos operandos (todos `Ref` → geral; todos `int`/`double`/`bool` → compacta; senão tags).
    Os dois `alloca` (palavras e tags) saem em `buffer_de_lista`.
26. **Ajudantes da P3 no prelúdio** (`listas_ir::AJUDANTES`, todos sem alocar nem lançar, em
    `EFEITOS_DOS_AJUDANTES`): os quatro de §3.5 mais `@df.lista_capacidade(l)`,
    `@df.lista_ler(a, i)` (`Ref`: a `CallRuntime` com retorno `Ref` é enraizada; uma
    `CargaNativa` seria `I64`), `@df.lista_gravar_ref(a, i, v)` (com `@df.barreira_elemento`),
    `@df.lista_gravar_len(l, n)` e `@df.lista_gravar_dados(l, d)` (grava com `@df.barreira` e
    devolve 1 se a forma de `d` é a do armazenamento atual; senão 0, e o native devolve a forma
    compacta ao armazenamento sem tipo que o `_shrink` da VM monta). Ganchos `pub(super)` em
    `lower/listas.rs`: `lista_len_em_linha`, `lista_armazenamento_em_linha` (`I64`),
    `lista_forma_em_linha`, `acrescentar_escalar_em_linha`.
27. **`lista_em_linha`:** `List_getLength`, `GrowableList_getLength`/`_getCapacity`/
    `_setLength` (sem barreira nem conferência, como a VM), `_setData` (acima),
    `List_setIndexed`/`GrowableList_setIndexed` (em linha só no armazenamento geral dentro da
    faixa; o resto no native) e o `[]` (`_Array`/`_List`/`_ImmutableList`/`_GrowableList`: em
    linha na geral dentro da faixa; compacta ou fora, `DartForge_lista_get`). O `add` escalar de
    `intrinsecos.rs` grava direto numa `_GrowableList` compacta da forma do valor com lugar,
    chama `dartforge_lista_acrescentar` cheia e o `add` do SDK no resto.
28. **Natives:** `Internal_makeListFixedLength`, `Internal_makeFixedListUnmodifiable` e
    `dartforge_collection_mark_unmodifiable` devolvem objeto novo com o tipo copiado
    (`tipo_lista_copiada`); o getter de constante (`constantes.rs`) já usava o valor devolvido e
    os chamadores do SDK (`array_patch.dart`, `string_patch.dart`, `array.dart`) também.
    `GrowableList_allocate` é o `_withData` da VM (compartilha o `_List`). `_grow` continua
    `DartForge_GrowableList_reservar` (armazenamento novo da mesma forma, uma cópia).
    `ClassID.numPredefinedCids` passa a `PRIMEIRO_CID_LIVRE` (128): as classes do runtime usam o
    `hashCode` no hash das constantes, como as predefinidas da VM (`compact_hash.dart`).
29. **Saem do runtime** (`nativos_listas.rs`): `dartforge_lista_cabecalho`,
    `_lista_len_gravavel`, `_lista_add_escalar` (§3.7). Ficam como transição, sobre a API nova:
    `dartforge_lista_len_ou_menos1` e `_lista_ref` (o primeiro ainda emitido pelo for-in de
    `lower/sdk_fonte.rs`), `dartforge_collection_is_unmodifiable` (usado só pelo legado de
    `colecoes.rs`). O legado de `colecoes.rs` (`dartforge_list_*`, `_map_*`, `_set_*`,
    `_iteration_*`, `_generic_len`) continua sobre a API velha até a P5 confirmar que ninguém o
    emite (hoje: `atribuicao.rs`, `expressoes.rs`, `membros.rs`, `padroes.rs`, `verificador.rs`,
    `erros_do_runtime.rs`, `comandos.rs`, `despacho.rs`, `ffi_callbacks.rs`); aí a P3 o apaga.
    `lower/literais.rs` perdeu os ramos sem SDK da fonte (o literal com elementos de controle
    acrescenta à lista por `dartforge_lista_acrescentar`); `laco_indice` e
    `ler_elemento_iteravel` ficam (a P4 os usa em `comandos.rs`).
30. **Pedidos da P3:** à P0, em `externs.rs`, tirar as declarações de `dartforge_lista_cabecalho`,
    `_lista_len_gravavel`, `_lista_add_escalar` e `_list_new` e marcar
    `dartforge_collection_mark_unmodifiable` como `ALOCA_SEM_LANCAR`; no for-in de
    `lower/sdk_fonte.rs` (≈2120–2160), trocar `dartforge_lista_len_ou_menos1` e
    `dartforge_lista_cabecalho` pela classe (`dartforge_value_class`, cid 8–10) e por
    `lista_len_em_linha` (ou os ganchos da P4, item 20). À P5, em `tipos.rs`, levar
    `ajustar_forma_da_lista`/`definir_tipo_da_lista` a `Heap::lista_ajustar_forma` (item 24) e
    manter `tipo_lista_da_tupla`/`tipo_lista_copiada` com a assinatura de hoje (a P3 passa o cid
    fixo como classe concreta). `Object_getHash` e `DartForge_verdadeiro` continuam em
    `nativos_listas.rs` (§3.8 dá `Object_getHash` à P2; §4.9 o exclui da lista da P5): quem os
    quiser os move pela regra de §4.9.
31. **P1 — `textos.rs` além de §3.3:** `TextoRef::cabe_em_um_byte`; em `Heap`,
    `escrever_texto(destino, pos, t)`, `copiar_texto(destino, pos, fonte, inicio, fim)` (sem
    cópia intermediária), `fatia_de_texto(h, inicio, fim)` (o `_substringUnchecked`) e
    `juntar_textos(&[Ref])` (concatenação numa alocação); `TextoMut::push_vista`;
    `IterUnidades` passou a andar sobre a vista e é `DoubleEndedIterator`. `Texto::hash_vm`
    continua `i64` (a API velha a usa); `TextoRef::hash_vm` é `u32`. O `Texto` e os demais
    saíram de `heap.rs` para `textos.rs` numa troca só (autorizada), com `heap.rs` reexportando.
    `hash_de_texto(&self)` grava o hash por `AtomicU32` e nunca num estático (`PERMANENTE`).
    `textos_iguais(a, a)` só é verdadeiro se `a` é string. `string_literal` interna por
    `Heap::literal`/`guardar_literal` (a P0 os acrescentou): o literal fica em `literais` (raiz) e
    `permanentes` — a `lista_de_tabela` da P3 depende disso.
32. **P1 — fragmentos:** `alocar_texto(Texto)`, `alocar_str`, `lancar_range` e `com_texto`
    ficam (usados por outros pacotes); `com_texto` passa a entregar a `TextoRef` (a API de leitura
    da `Texto` é a mesma). Saem `texto_de`, `texto_de_qualquer`, `dartforge_texto_dados`,
    `_texto_len` e `_texto_na_falha`. Novos: `inteiro_do_valor(heap, Valor)` (`Smi`, `_Mint` ou
    `Valor::Int`), `copia_de_texto`, `texto_copiado`, `novo_acumulador_com(Vec<u16>)` e
    `acrescentar_ao_acumulador`; em `regexp.rs`, `novo_programa_re`,
    `padrao_do_programa_re(heap, h) -> (Vec<u16>, [bool; 4])` e
    `programa_re_de_padrao(fonte, opcoes)` para a cópia entre isolados (§2.12; a P5 já lê o
    acumulador direto do anexo em `portas.rs`, o `Box<Vec<u16>>`).
33. **P1 — natives recebidos (§4.9):** além dos da tabela, `DartForge_string_codeUnitAt` (o
    caminho lento de `codeUnitAt`) também veio de `nativos_listas.rs` para `nativos_strings.rs`.
34. **P1 — `RegExp`:** `_RegExp._id` virou `final Object _programa` (o `_ProgramaDeRegExp`);
    `_compilar` devolve `Object?` (null no erro); os natives recebem o handle. `ProgramaRe`
    guarda o padrão (`fonte`). Sai `PROGRAMAS_RE`. O legado `dartforge_regexp_new` compila pelo
    mesmo motor (o casador rudimentar saiu) e `dartforge_string_split_map_pieces` devolve a
    parte casada como `String` (o `Value::Match` não existe mais).
35. **P1 — `StringBuffer`:** o acumulador conta para os gatilhos só o que tinha ao ser criado
    (`anexar(.., bytes)`). O crescimento do `Vec` não entra em `contar_externos`, porque a morte
    desconta só os `bytes` registrados. **Pedido à P0:** `Heap::ajustar_anexo(h, bytes)` para o
    dono atualizar o tamanho registrado; o P1 passa a chamá-lo em `sb_escrever*`.
36. **P1 — emissor:** mais um ajudante, `@df.texto_igual_a(a, b)` (o `_StringBase.==` com um
    `Object?`: identidade, `e_objeto`, cid 6/7, depois `@df.texto_igual`). `@df.texto_igual` não
    compara cids (uma `_TwoByteString` Latin-1 do `allocateTwoByteString` pode ser igual a uma
    `_OneByteString`): o falso rápido vem só do comprimento ou dos dois hashes calculados e
    diferentes. Os ajudantes são texto fixo em `AJUDANTES`, conferido contra `layout` por
    `const` e por teste. `lower/textos.rs` chama os ajudantes por `CallRuntime df.texto_*`
    (efeitos em `textos_ir::EFEITOS_DOS_AJUDANTES`) e faz em linha também
    `DartForge_string_igual_a`/`_iguais`. O campo `caches_de_texto` saiu de `lower/fn_builder.rs`
    (autorizado). O literal estático é `@"df.s.<32 hex do blake3(cid, unidades)>"`, com o
    `comdat` escrito junto do global em `emitir_globais_de_texto`.
37. **P1 — `integers.dart`/`double.dart`** (item 7 de §4.3): ficam como estão até a medição
    da integração. Não houve execução antes do corte, e a troca pelo `_Smi.toString` da VM com
    `@df.texto_alocar`/`@df.texto_gravar` em linha só se decide medindo.
38. **Pedidos da P1:** à P0, em `externs.rs`, tirar as declarações de `dartforge_texto_dados`,
    `_texto_len` e `_texto_na_falha` (o runtime não as define mais; o teste
    `cada_extern_do_emissor_existe_no_runtime` acusa), e `ajustar_anexo` (item 35). Aberto para
    a P0b: o literal estático em `comdat` só é único dentro de uma ligação. Entre o executável e
    a DLL do SDK há duas cópias, então `identical` de literais iguais entre o programa e o SDK
    depende da mesma solução do problema 9(a).
39. **P5 — exceção pendente:** `excecoes.rs` guarda a exceção como o par da ABI plana `(bits,
    tag)` numa `TaggedValue` (fora do heap) até o corte, porque `nucleo.rs` (P2) a lê no fim do
    programa com `untag`; `excecoes.rs` já não usa `tagged`/`untag`/`valor_como_ref` (tem
    `excecao_da_abi`, `etiqueta_da_excecao`, `excecao_como_ref` sobre `como_ref(Valor)`). No
    corte, `EXCEPTION` passa a `Option<Valor>` junto com esse leitor.
40. **P5 — compatibilidade que fica até o corte** (para chamadores de outros pacotes):
    `seletores.rs` mantém `cid_registrado(pos)` e as posições `CID_*` (agora o cid fixo da
    posição, `CIDS_FIXOS_POR_POSICAO`; `ffi.rs`, `io_arquivos.rs`, `simd.rs`), `cid_do_runtime(h)`
    (= `Heap::classe`; `nativos_listas.rs`, `io_plataforma.rs`, `nucleo.rs`) e
    `cid_do_valor_do_runtime` (genérico no valor, que ignora; `nucleo.rs`); sai
    `CIDS_DO_RUNTIME`. `portas.rs` mantém `Grafo::escalar(bits, ValueTag)` (`io_eventos.rs`) ao
    lado dos novos `Grafo::nulo()`/`Grafo::de_int(v)`. O `use crate::heap::{smi, ValueTag}` de
    `tipos.rs` continua sendo o import desses nomes para todos os fragmentos.
41. **P5 — `dartforge_registrar_cids`:** aceita as duas tabelas que o emissor pode passar — os
    65 cids de `DO_SDK` em ordem, ou as 32 posições de `sdk_modulo::cids_do_runtime` (com -1 para
    a classe que o SDK carregado não tem) — e aborta com a mensagem se a tabela do módulo não é a
    do runtime. `dartforge_definir_migracao` descarta (com mensagem) a entrada de cid < 128.
42. **P5 — RTI:** `chave_do_valor` é `metadado` ou `−16 − cid` para todo valor (null −17,
    `Smi` −18), menos o `_Record` sem metadado (o tipo sai dos campos: sem chave).
    `rti_definir` não grava num estático (`PERMANENTE`) e ajusta a forma de toda lista do núcleo
    (cid 8–10); `ajustar_forma_da_lista` enraíza a lista durante `lista_ajustar_forma`, que pode
    alocar (pedido da P3).
43. **P5 — cópia entre isolados:** os nós do grafo são `Instancia` (campos com o bit),
    `Refs` (palavra 0 e referências), `Compacta` (lista `BRUTO` com a forma), `Bruto`
    (`_Mint`, `_Double`, SIMD), `Texto`, `Tipada` (interna ou externa → interna), `Visao` (base
    copiada, dados recalculados por `nova_visao`), `Acumulador` e `ProgramaRe`; os dois anexos
    são criados pelo dono (`novo_acumulador_com`, `programa_re_de_padrao`, P1) antes do empréstimo
    do heap, já enraizados no quadro da mensagem. Outro anexo chega como `null`. `ValG::Estatico`
    leva um objeto `PERMANENTE` pelo endereço; a thread de um serviço nativo, sem heap, lê o
    estático direto pelo `layout` (`bool` e strings). As listas usam `nova_lista` (cartões) e
    `gravar_refs`; a lista compacta copia as palavras antes de publicar.
44. **P5 — emissor sem o modo legado:** `lower/sdk_por_nome.rs` e `lower/nsm.rs` foram apagados
    (nenhum chamador; o encaminhador em linha de `nsm.rs` só valia sem o SDK da fonte; a P0 tirou
    os `mod` de `lower/mod.rs`). O despacho pelo nome em mundo fechado de `lower/despacho.rs` (`Alvo`, `Uso`,
    `alvos_por_nome`, `alvos_de_escrita`, `despachar`) saiu; o membro sem elemento útil vai por
    `metodo_por_seletor` (`chamadas.rs`) e `propriedade_por_seletor` (`expressoes.rs`). O padrão
    de record posicional confere o cid 12 e lê pelos natives `DartForge_record_numFields`/
    `_fieldAt` (saem `dartforge_record_len`/`_get_ref`); `String * n` vai pelo seletor (sai
    `dartforge_string_repeat`); `identical` de `Ref` é `@df.identico` e o `==` com `String` não
    anulável à esquerda é `@df.texto_igual_a`. O record posicional de `expressoes.rs` é
    `lower_registro_posicional` (P2). `escape.rs` só substitui objetos de cid ≥ 128;
    `simplificar.rs` trata como puros os ajudantes `df.*` das quatro tabelas de efeitos que não
    lançam nem gravam (pedido da P4).
45. **P5 — verificador da HIR:** recusa `AllocMap`/`AllocSet`, record posicional com campo que
    não é `Ref`, e captura lida numa representação diferente da gravada (`EnvGet` sobre o
    parâmetro `env` de um corpo de closure, contra o `AllocEnv` passado a `AllocClosure`/
    `AllocClosureTipada` no módulo; `CellGet`/`CellSet` contra a `_Celula` da mesma função ou a
    lida do ambiente). Os pares `(bits, tag)` conferidos ficaram só os da ABI de lançamento.
46. **P5 — natives recebidos (§4.9):** `Object_equals`, `_toString`, `_haveSameRuntimeType`,
    `_runtimeType` e `WeakReference_*`/`WeakProperty_*` em `nativos_sistema.rs` (movidos por quem
    recebe, regra do coordenador).
47. **P5 — testes:** `EN/tests/contrato.rs` e `crates/jit/tests/sessao_persistente.rs` no modo
    com SDK da fonte (três asserções sobre `print_i64`, que só o `print` por nome emitia, saíram);
    `EN/llvm/testes.rs` ganhou o literal estático e as regras novas do verificador;
    `crates/cli/tests/reload_estado.rs` perdeu a variante sem SDK e ganhou
    `cli_preserva_o_estado_do_espaco_unificado_em_tres_recargas` (§5.2 item 9, fixtures
    `reload_espaco_v1..4.dart`, saída conferida na VM); os testes da CLI não passam mais
    `DARTFORGE_SDK_DA_FONTE`. Os programas 90–99 estão em `corpus/nativo` (o 92 é um diretório
    com duas bibliotecas).
48. **Pedidos da P5:** à **P0** — (a) tirar `pub mod sdk_por_nome;` e `pub mod nsm;` de
    `lower/mod.rs` (a P5 então apaga os dois arquivos); (b) em `sdk_fonte.rs`,
    `gerar_encaminhador_nsm`, encaixotar os valores (`coagir(v, Type::Ref)`) antes do
    `AllocEnv`: `dartforge_encaminhar_nsm` lê todas as capturas como `Ref`; (c) tirar
    `Context::sdk_da_fonte`, `sdk_da_fonte_pedido`/`DARTFORGE_SDK_DA_FONTE` e o `bool` de
    `emitir_ir_com`/`compilar_com`, com os ramos que sobram em `closures.rs`, `comandos.rs`,
    `constantes.rs`, `fn_builder.rs`, `intrinsecos.rs`, `literais.rs`, `mod.rs`, `registros.rs`,
    `sdk_fonte.rs`, `simd.rs` e `tipados.rs` (de cada dono); (d) tirar `AllocMap`/`AllocSet` da
    HIR (ninguém mais os emite); (e) tirar de `externs.rs` as 134 declarações que nenhum código
    do emissor cita mais (conferido por busca em 2026-09-30, fora `externs.rs` e
    `llvm/testes.rs`): as de acesso de §3.7 (`box_*`, `cell_*`, `closure_cabecalho`/`_code`/
    `_env`, `env_*`, `lista_ref`, `typed_ptr`, `record_len`/`_get_ref`), as do legado
    (`list_*`, `map_*`, `set_*`, `string_*` menos `new`/`concat`/`juntar_tipado`,
    `string_buffer_*`, `regexp_new`, `int_to_radix_string`, `print_list`/`_map`/`_set`,
    `dyn_unario`), e outras que já não eram emitidas (§1.7). A lista exata sai de
    `grep -o '@dartforge_[a-z_0-9]*' llvm/externs.rs` contra o resto de `src/`; os runtimes que
    as definem (`strings.rs`, `colecoes.rs`, P1/P3) as apagam junto. À **P2** — `nucleo.rs`: ler a
    exceção pendente pelo par da ABI no corte (item 39) e trocar `cid_do_valor_do_runtime`/
    `cid_do_runtime` por `Heap::classe`. À **P3** — `nativos_listas.rs`: `cid_do_runtime(h)` →
    `Heap::classe`. À **P4** — `io_eventos.rs`: `Grafo::escalar(x, ValueTag::Int)` →
    `Grafo::de_int(x)`; `ffi.rs`, `io_arquivos.rs`, `simd.rs`: `cid_registrado(CID_*)` →
    `layout::cid`; `io_plataforma.rs`: `cid_do_runtime` → `Heap::classe`.
49. **P0b — o espaço (`espaco.rs`).** Classes de tamanho por palavras do corpo (`classe_de_tamanho`:
    1–64 exatas e as 24 médias, índice da classe = palavras nas exatas); a TLAB de `w` palavras é
    a classe `w`. Região grande: `bytes_da_regiao(w)` = mapa de marcas + bloco, arredondado a 4 KiB,
    alinhado a 64 KiB (`VirtualAlloc(MEM_RESERVE | MEM_COMMIT)` no Windows; `mmap` com o recorte em
    todo Unix, `MAP_ANON` 0x20 no Linux e 0x1000 nos demais), registrada no `MapaDePaginas` como
    uma página de um bloco; a morta vai a um cache por tamanho (`RegioesGrandes`), reusada zerada, e
    as paradas de uma coleta completa à outra voltam ao sistema. Anexos por endereço do bloco
    (`HashMap`), soltos na varredura (menor e completa) pelo bit de marca, e no fim do heap.
50. **P0b — contratos que o texto não fixava.** (a) `dartforge_alocar(cid, palavras, x)`: `x` são os
    bits 8–31 da palavra 0 do cabeçalho que o código montou (`cabecalho >> 8`): `flags` nos 8 de
    baixo e, em `INSTANCIA`, o número de campos nos 16 seguintes (em `BRUTO`/`REFS` o `n` é
    `palavras`). (b) `REFS`: a palavra 0 é o **número de `Ref` que seguem** (o comprimento da
    `_List`, os campos do `_Record`); o coletor percorre as palavras `1..=min(palavra 0, w − 1)` —
    as palavras de cartões vêm depois dos elementos e não são `Ref`. (c) `Heap::alocar(cid, w,
    flags)` grava `n = w`; `INSTANCIA` com mapa estendido é por `alocar_instancia`. Quem aloca uma
    `REFS` grande acende `CARTOES` e reserva as palavras (`layout::palavras_de_lista`). (d)
    `Heap::palavras(h)` começa em `b+16` (a palavra 0 é a do comprimento); em `INSTANCIA`, é o
    corpo (o de fora, se houver). (e) `gravar_refs` num velho com `CARTOES` suja os cartões da
    faixa inteira e lembra o objeto se algum valor é jovem; `lembrar(h)` suja todos.
    (f) `Heap::ajustar_anexo(h, bytes)`: a diferença conta nos gatilhos.
51. **P0b — estáticos.** Um handle de objeto fora das páginas é estático se cai numa faixa
    registrada (`dartforge_registrar_imagem(inicio, fim)`, `heap::registrar_imagem`) ou é uma das
    caixas de `bool` do runtime, e o estado é `PERMANENTE`; nada lê memória fora dessas faixas.
    O estático é legível por toda a API (`cabecalho`, `classe`, `palavras`, `metadado`,
    `e_objeto_vivo`, `hash_de_identidade`); gravar nele é pânico (`conferir_gravavel`). O
    registro da imagem: cada módulo com `objetos_estaticos` escreve os marcadores da seção
    (`alvo::marcadores_da_imagem`: `.dfimg$a`/`$z` em `comdat` no COFF, `__start_dfimg`/
    `__stop_dfimg` com uma âncora no ELF, `section$start/end$__DATA_CONST$__dfimg` no Mach-O) e
    chama o registro na função de registro dele (`df.registrar.<uri>` e `df.registrar.programa`);
    `alvo::secao_da_imagem()` é a seção dos literais. `objetos_estaticos`: `emitir_ir`/`compilar`
    (AOT) e os módulos do SDK, sim; `emitir_ir_recarregavel` (JIT), não. Com eles, o literal não
    divide o bloco (`rotulos_de_saida`).
52. **P0b — problema 9 (a)/(b), `bool`.** O código gerado não referencia mais os dados
    `dartforge_falso`/`_verdadeiro`: o `Contexto` da thread publica os dois handles
    (`layout::contexto::VERDADEIRO` = 360, `FALSO` = 368) e `@df.caixa_bool`/`@df.desencaixa_bool`
    (`caixas_ir.rs`, mudança mecânica autorizada) os leem dali. Só o runtime ativo tem `Contexto`,
    então entre o executável e a DLL do SDK vale sempre a cópia dele; o JIT não precisa publicar
    dados. `externs::GLOBAIS` ficou vazia. Continua aberto (9a, item 38): um literal de string
    igual no programa e no SDK é um estático em cada imagem na ligação de desenvolvimento
    (executável + DLL); `identical` entre os dois só na produção, em que tudo é uma imagem.
53. **P0b — emissor.** `@df.classe` sem o ramo de slot: `0` → 1, ímpar → 2, objeto → carga
    `!invariant.load`, o resto (handle inválido) → `dartforge_value_class`, que dá o pânico N4.
    `@df.alocar`, `@df.barreira`, `@df.barreira_elemento`, `@df.e_objeto` e `@df.filho_jovem`
    (em `ajudantes_do_espaco`, com os deslocamentos de `layout`); a alocação em linha de
    `dartforge_object_new` pela TLAB de `max(n, 1)` palavras; a barreira de campo por
    `@df.barreira(o, v)`. Pendente para a integração: remedir `LIMITE_JOVEM` e `CONTAGEM_JOVEM`
    (passo 10), que exige executar.
54. **Corte (P0, 2026-09-30).** Saíram do runtime: `enum Value`, `TaggedValue`/`ValueTag`,
    `Elementos`/`CabecalhoDeLista`/`FormaDeLista`/`Armazenamento`/`CabecalhoTipado`/
    `CabecalhoDeClosure` e os vazios, `ClasseDoSlot` e as marcas, os campos de slot do `Heap`
    (`slots`, `metadados`, `free`, `marks`, `idade`, `slots_jovens`, `slots_lembrados`,
    `bytes_do_slot`, `hashes_de_texto`, `classes`, `cids_do_runtime`, `caixas_bool`,
    `imutaveis_do_espaco`, `marcador`, `iteracoes_ativas`, `origens`) e os métodos de §2.17 com
    os `*_velho`; o N4 passou a `bloco_vivo`/`handle_invalido` (as quatro mensagens, com "já
    coletado" quando o endereço cai numa página do espaço). A exceção pendente é
    `Option<heap::Valor>` (`excecoes.rs`: `bits_da_excecao`/`etiqueta_da_excecao`, lidos por
    `nucleo.rs`). `portas.rs`: `NoG::DoRuntime`/`Portavel::DoRuntime` levam o `cid` fixo;
    `Grafo::escalar` saiu. `seletores.rs`: saíram `cid_registrado`, `cid_do_runtime` e
    `cid_do_valor_do_runtime` (fica `CIDS_FIXOS_POR_POSICAO` para a conferência de
    `dartforge_registrar_cids`). `colecoes.rs` ficou só com
    `dartforge_collection_mark_unmodifiable`. Saíram 80 funções exportadas que nenhum código
    cita (o legado sem SDK da fonte e os acessos da representação velha; ficam as 5 que os testes
    usam: `closure_code`, `print_bool`/`_f64`/`_string`, `rti_subtipo`), e do emissor `AllocMap`/
    `AllocSet` e 138 declarações de `externs.rs`. Os testes do módulo `heap` foram reescritos
    sobre a API nova (`tests`, `espaco_de_objetos`, `falhas_de_handle`, `raizes_do_runtime`,
    `review_tests`, `fixed_root_tests` e os novos de `espaco_unificado`); saíram os que só
    testavam a representação velha (`texto_utf16` — o `Texto` é de `textos.rs` —, `smi_r10`,
    `caixas`, `maps_sets_tearoffs`, `captures_and_lists`).

**Integração (2026-09-30):** o que a primeira execução de ponta a ponta exigiu além do texto.

55. **`anexar` num bloco que já nasce com `ANEXO`.** Os donos alocam o acumulador e o programa
    de `RegExp` com `alocar(cid, 1, BRUTO | ANEXO)` (§2.4); `Heap::anexar` recusava a flag já
    acesa ("anexo duplicado", 67 programas). O duplicado passa a ser a palavra do ponteiro já
    gravada (`b+16 ≠ 0`).
56. **Cartão da barreira de elemento.** `@df.barreira_elemento` somava `desl::ELEMENTOS` (24,
    do bloco) ao handle, 2 bytes além: o `or` do cartão caía desalinhado e a coleta menor não
    via o elemento novo (lista velha grande com filho jovem morto e reusado: 96, 99). O
    deslocamento é `ELEMENTOS − 2`, como os demais do contrato (`d(desl::…)`).
57. **`int.hashCode` é o da VM, não o valor** (§2.10 supunha o valor; o 95 fixa `7.hashCode ==
    81207`): o `HashIntegerOp` (`il_x64.cc`), o produto de 96 bits de `v` sem sinal por
    `0x2d51` com as três palavras de 32 bits combinadas por xor, cortado a 30 bits. Em linha
    em `lower/caixas.rs` (`DartForge_int_hashCode`), no native (`nativos_numeros.rs`) e no
    caminho rápido do `_Map`/`_Set` (`nativos_hash.rs::chave_de_hash`, que tem de dar a
    mesma tabela que o Dart). `double.hashCode`: o do `int` quando é inteiro e cabe em 64
    bits; senão `(b ^ b >>> 32) & (2^62 − 1)` (a VM).
58. **Literal canônico = o estático da imagem.** `identical(s[0], 'a')` (o `String_charAt`
    da VM devolve o símbolo predefinido) e a `const` de string montada (`const x =
    'a${'b'}'`, que o emissor avalia em tempo de execução) precisam dar o literal estático de
    mesmo conteúdo, que o runtime não conhecia. `textos.rs` ganhou um índice dos estáticos
    das imagens registradas, montado sob demanda pelo hash já gravado no cabeçalho
    (`estatico_de_texto`; percorre a seção pulando palavras zero e para no primeiro cabeçalho
    que não é string estática); `Heap::string_literal` o consulta antes de internar, e o
    getter de `const` (`constantes.rs`, `membros.rs`) passa o valor por
    `dartforge_constante_canonica` (string → literal canônico; o resto como está). Entre o
    executável e a DLL do SDK com o mesmo literal vale o registrado por último (o programa).
    `heap::imagens()` dá as faixas.
59. **O JIT recebe IR sem estáticos.** O diferencial (`--jit`, `--jit-aot`) e os testes do JIT
    mandavam ao JIT o IR do AOT (`emitir_ir`, com `.dfimg`): no JIT os marcadores `$a`/`$z`
    não delimitam a seção e todo literal era "handle além da tabela". Passam a usar
    `emitir_ir_recarregavel(.., None)`, o do `dartforge run` (`oraculos::dartforge_nativo_ir_jit`,
    `jit/tests/execucao.rs`, `sessao_persistente.rs`); o `--jit-aot` liga esse mesmo IR.
60. **`d op= n` com `d` `double` e `n` `num`.** `lower_binary_op_helper` desencaixava o `num`
    como `_Double` (`TypeError` para um `int`; defeito anterior, exposto pelo 96). Novo
    ajudante `@df.num_para_double` (`caixas_ir.rs`): `Smi`/`_Mint` convertidos, o resto pelo
    `@df.desencaixa_double`.
61. **Externs compostos pelo nome.** O corte tirou `dartforge_typed_novo_t`, `_view_nova_t` e
    `_typed_externo_t` (emitidos como `@{name}_t` em `llvm/mod.rs`, invisíveis à busca por
    nome); voltaram em `externs.rs`, `typed_data.rs` e `ffi.rs`.
62. **`--gc-stress` sem custo quadrático.** Uma completa a cada 8 coletas custa o heap vivo
    inteiro a cada 8 alocações: com os novos programas (e todo valor sendo bloco) o 99 passava
    de 6 min. No estresse, a completa vem a cada `max(8, trabalho da última completa / 64)`
    coletas (`TRABALHO_POR_MENOR_NO_ESTRESSE`), e a verificação (`DARTFORGE_GC_VERIFICAR`)
    com o mesmo espaçamento; a menor continua antes de toda alocação. A mensagem do
    verificador passou a dar cid, estado, flags e posição do pai.
63. **Handle sem conferência no mapa de páginas (desvio de §3.2, "pânico N4").** Medido, o
    runtime passava 65% do tempo de `Set<String>.add` em `bloco_vivo`/`bloco_de` (o mapa de
    páginas, a `Pagina`, o índice do bloco) a cada leitura de cabeçalho. Agora a conferência
    completa, com as mensagens N4 de handle inválido, vale com `validar_handles`: no
    `--gc-stress`, com `DARTFORGE_GC_VERIFICAR=1`, nos testes (`cfg(test)`/`debug_assertions`)
    ou com `DARTFORGE_VALIDAR_HANDLES=1`. Fora disso um handle de objeto é o bloco (a
    invariante do código gerado e do runtime) e só o estado `LIVRE` é conferido, na leitura do
    cabeçalho que se faz de todo modo ("já coletado" continua); um escalar usado como handle
    derruba o processo por acesso inválido em vez da mensagem. `Heap::bloco_do_espaco` é o
    `objetos.bloco_de` dos dois modos (nunca um estático); `bloco_vivo` é `inline(always)`
    com o caminho conferido `#[cold]`. Efeito (60–100 rodadas): `Set<String>` 59→38 ms,
    `Map<int,int>` 185→118 ms, `textos/hashes` 64→34 ms, `construir` 115→72 ms.
64. **Palavras do objeto grande em `mapa`.** O `n` satura em `u16::MAX` (corpo acima de 512 KiB)
    e `palavras_do_corpo` ia ao mapa de páginas a cada leitura (13% do `Map<int,int>` de 500
    mil entradas, cujo `_data` tem 1 milhão de palavras). Um objeto grande que não é string nem
    `INSTANCIA` (onde `mapa` não tem uso, §2.3) guarda em `mapa` as palavras do corpo
    (`EspacoDeObjetos::alocar`); a string grande continua pela região.

---

## 5. Testes e medições (no fim)

### 5.1 O que quebra, e quem conserta

* **Tudo o que executa código gerado**, até a integração: a árvore compila (§4.1), mas o runtime
  tem `todo!()` até cada pacote terminar.
* **Testes de unidade do `heap`** (`RT/heap.rs` 4931-6216, ~70 testes sobre `Value`, slots e
  coleções legadas): reescritos por P0.
* **`EN/tests/contrato.rs:36`** e **`crates/jit/tests/sessao_persistente.rs:148`** compilam no
  modo sem SDK da fonte: passam ao modo com SDK (P5).
* **`EN/llvm/testes.rs`**: usa externs de string que saem (P5).
* **`crates/runtime/tests/tipos_e_eventos.rs`** usa só a ABI (`dartforge_string_new`,
  `dartforge_object_new`): continua valendo, com o cid do objeto ≥ 128 (P0).
* **Oráculos em cache do diferencial**: a saída da VM não muda; os binários sim.

### 5.2 Sequência de validação

Máquina Windows 11 (ambiente do scratchpad `agente-http/env.sh`), `--release`:

1. `cargo build --release -p dartforge-cli --features nativo`.
2. `cargo test --release -p dartforge-runtime -p dartforge-emit-native -p dartforge-jit -p dartforge-cli`
   (inclui `tests/hot_reload.rs`, `reload_estado.rs`, `io_regressao.rs`, `jit_programa.rs`).
3. Módulo `heap` com `DARTFORGE_GC_VERIFICAR=1`.
4. `env -u HOME cargo run --release -p dartforge-diferencial -- --nativo --corpus corpus/nativo`
   (82 programas de hoje + 90–99).
5. O mesmo com `--jit` e `--jit-aot`.
6. O mesmo com `--nativo --gc-stress --limite-exec 60`, uma vez sem e uma com
   `DARTFORGE_GC_VERIFICAR=1`.
7. `--nativo --corpus corpus/js` (235 programas) e `--nativo --producao` (o `aot --optimize`).
8. `bench/desempenho/*.dart`, `bench/simd` e `bench/http/servidor.dart` com a saída igual à da VM.
9. Recarga: um programa que guarda strings, listas compactas e gerais, closures, `double` em
   caixa e uma lista grande com cartões em globais, recarregado três vezes (`crates/cli/tests/
   reload_estado.rs` ganha o caso), com a saída igual antes e depois.

### 5.3 Programas novos (`corpus/nativo`, P5 escreve, os pacotes conferem)

| Nº | Arquivo | Cobre |
|---|---|---|
| 90 | `90_textos_um_e_dois_bytes.dart` | `_OneByteString`/`_TwoByteString` (latin-1 no limite 0xFF, CJK, emoji), pares substitutos e soltos, `substring` no meio do par, `codeUnitAt`, `runes`, `toUpperCase` que troca de forma, `print` de solto (U+FFFD) |
| 91 | `91_textos_grandes_e_interpolacao.dart` | concatenação e interpolação até 1 MiB (classes médias e região grande), `StringBuffer` de 5 MiB, `*`, `padLeft`, `split`/`join` grandes, `writeCharCode` de par |
| 92 | `92_textos_identidade_hash_mapas.dart` (+ `92_textos_parte.dart` importado) | `identical` de literais iguais entre bibliotecas e em `const`, de string montada (falso), `hashCode` estável e igual ao da VM, `==` entre formas, strings como chaves de `Map`/`Set` (100 mil), `switch` sobre string |
| 93 | `93_textos_entre_isolados.dart` | strings pequenas, grandes e de dois bytes por `SendPort`, `Isolate.run`, `Isolate.exit`, listas e mapas de strings, literal enviado e comparado com `identical` no destino |
| 94 | `94_utf8_e_bytes.dart` | `utf8.encode`/`decode` (inválido com `allowMalformed`, BOM, surrogates), `latin1`, `String.fromCharCodes` de `Uint8List` e de visão, `json.fuse(utf8)` |
| 95 | `95_caixas_closures_records.dart` | `identical(1.0, 1.0)`, `-0.0`, `NaN`, `_Mint` nos limites do `Smi`, `double` em `List<Object>`, closures que capturam `int`/`double`/mutáveis, tear-offs iguais e diferentes, `Function.apply`, records posicionais e nomeados (`==`, `hashCode`, `toString`) |
| 96 | `96_listas_formas_e_cartoes.dart` | `List<int>`/`<double>`/`<bool>` compactas e a descompactação, `List<Object?>`, crescimento, `List.filled`/`generate`/`unmodifiable`/`of`, erros de lista fixa e imutável, `sublist`, `sort`; uma lista de 100 mil elementos velha recebendo objetos novos em pontos espalhados entre coletas (cartões) |
| 97 | `97_mapas_conjuntos_grandes.dart` | `Map<String, int>` e `Set<int>` com 200 mil entradas, remoção e reinserção, iteração com modificação (erro), `const` mapas e conjuntos, `Map.unmodifiable` |
| 98 | `98_tipadas_visoes_externas.dart` | listas tipadas de todos os tipos, visões e visões não modificáveis, `ByteData` com endian, `sublistView`, listas maiores que 16 KiB, SIMD em lista; `asTypedList` sobre `malloc` de `dart:ffi` (`DynamicLibrary.process()`, onde houver) |
| 99 | `99_pressao_de_coleta.dart` | milhões de strings, listas e caixas curtas e longas, estruturas que sobrevivem e morrem por rodada, gravação velho→jovem em listas, mapas e closures; roda com `--gc-stress` e `DARTFORGE_GC_VERIFICAR=1` dentro de `--limite-exec 60` |

### 5.4 Medições

Método (o de NATIVO-PLANO §9.10): execuções alternadas DartForge × Dart AOT
(`dart compile exe`, `C:\tools\dartsdk-3.6.2`), mínimo e mediana, razão por par; perfil por
amostragem com `scratchpad/agente-http/pilhas.py` e o simbolizador pelo mapa do `lld-link`.
Antes = o `main` do começo da integração; depois = o do fim.

| Medida | Hoje | Estimativa depois (não medida) | Dart AOT |
|---|---:|---|---:|
| `json.dart` `decode_medio` | 394 ms (7,2×, NATIVO-PLANO §10.4) | 2–3× | 54,6 ms |
| `json.dart` `encode_medio` | 149 ms (2,6×) | 1,5–2× | 57,3 ms |
| `json.dart` `decode_grande` | 509 ms (4,0×) | 1,5–2,5× | 127 ms |
| `textos/construir` | 159 ms (≈2×) | 1,0–1,3× | 81 ms |
| `textos/hashes` | 63 ms (1,7×) | ≈ 1,2× | 38 ms |
| `int.toString` × 1 milhão | 140 ms | 40–60 ms | 20 ms |
| `codeUnitAt` de campo, 1,5 milhão | 4,8 ms | ≈ 3,5 ms | 3,3 ms |
| servidor HTTP, CPU/req, 1 conexão | 180 µs | 140–155 µs (−15–20%) | 102 µs |
| string `"abc"` em memória (calculado) | ≈ 90 B (`Option<Value>` de 48 B, 26 B nos vetores paralelos, o bloco do `Vec` no `malloc`) | 32 B | 32 B (`OneByteString::InstanceSize`, `object.h:10902-10917`) |

Base das estimativas: no perfil do JSON (`PLANO-TAMANHO-DESEMPENHO.md` §2; NATIVO-PLANO §10.5),
alocação e coleta ~21%, classe do receptor ~12%, `malloc` 7–11% e consultas de string ao runtime
~4% são quase todos custo do slot; o que sobra é o parser em Dart, a RTI e o despacho.

---

## 6. Riscos e mitigação

| # | Risco | Mitigação |
|---|---|---|
| 1 | **Tudo quebra junto**: nada roda até a integração, e o primeiro defeito aparece tarde | contrato congelado (§3) e árvore sempre compilando (§4.1); P0 testa o coletor sozinho (§4.2, passo 9); cada pacote escreve testes de unidade da sua vista; a integração segue §5.2 na ordem, do teste de unidade ao corpus |
| 2 | **Barreira faltando** em gravação nova do runtime (quem escreve por `palavras_mut` num objeto publicado) | toda gravação de `Ref` por `gravar_ref`/`definir_campo`/`gravar_celula`; `palavras_mut` só em `BRUTO` ou antes de publicar (revisão por busca); `DARTFORGE_GC_VERIFICAR` com cartões; programa 99 |
| 3 | **Estático tocado pelo coletor** (o mapa de marcas de um endereço fora do heap) | `PERMANENTE` conferido antes de qualquer acesso ao mapa (§2.8); teste de unidade; seção só leitura (gravar num estático derruba o processo em vez de corromper) |
| 4 | **Literal duplicado entre módulos** (`identical` falso) | `linkonce_odr` + comdat pela chave do conteúdo; programa 92 com duas bibliotecas; conferido no `aot --optimize` (o `/opt:safeicf` não funde objetos com endereço tomado) |
| 5 | **Estático em memória que some** (geração do JIT liberada, J02) | sem estáticos nos módulos do programa no JIT (§2.11); a DLL do SDK fica carregada o processo inteiro |
| 6 | **Cid mudando depois de publicado** com `!invariant.load` | regra do §2.16 (objeto novo em vez de troca); `definir_flags` recusa mexer em outra coisa que `FORMA`/`ELEMENTO` |
| 7 | **Conversão de forma com coleta no meio** (caixas alocadas enquanto a lista ainda diz `BRUTO`) | a ordem de §2.16: montar o geral enraizado, depois copiar e trocar `flags` sem alocar |
| 8 | **`mapa` lido como mapa de referências numa string** | regra de §2.3 (`FORMA` antes de `mapa`), `debug_assert!` em `empilhar_referencias` e na cópia entre isolados |
| 9 | **Fragmentação** com 89 classes (uma página parcial por classe em uso: até ~5,6 MiB) | a folga por classe de hoje (`varrer`, `RT/heap.rs:2253`); medir o RSS de pico em `bench/desempenho` e no servidor; se preciso, juntar classes médias vizinhas |
| 10 | **Objetos grandes frequentes** (uma chamada ao sistema por objeto acima de 16 KiB) | cache de regiões soltas por tamanho (§4.2, passo 6), como o cache de páginas da VM; os buffers de 8 KiB do `dart:io` ficam em classe média |
| 11 | **`double` em lista geral e em record passa a ter caixa** (antes ficava sem caixa na `TaggedValue` e era reencaixotado a cada leitura como `Ref`) | é o modelo da VM; as listas `List<double>` ficam compactas; medir `json.dart` (listas de `double` do `_JsonListener`) |
| 12 | **Captura lida em representação diferente da gravada** (o `@df.env_ref` de hoje escondia isso) | `EnvGet`/`CellGet` no tipo gravado (P2) e o verificador da HIR recusando a divergência (P5) |
| 13 | **Outras frentes na mesma árvore** (vetorização em `comandos.rs`/`tipados.rs`; classe do valor, cujo `Heap::classes`/`ClasseDoSlot` esta especificação apaga) | P4 começa depois da vetorização; o trabalho de classe do valor é absorvido (o cid no cabeçalho substitui o vetor) |
| 14 | **Ponteiros do runtime para dentro de objetos** (FFI, TLS) atravessando alocação | regra de §2.14 (vale até a próxima alocação se o objeto não tem raiz); `com_raizes` onde o ponteiro atravessa alocação |
| 15 | **`--gc-stress` mais lento** (todo valor passa a ser objeto do espaço, sem TLAB no estresse) | `--limite-exec 60`; o programa 99 dimensionado para caber |
| 16 | **Mensagem entre isolados** com anexo ou lista externa | casos especiais de §2.12 e o programa 93 |
