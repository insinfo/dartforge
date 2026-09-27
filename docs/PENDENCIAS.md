# Pendências da auditoria (checklist de 2026-09-26)

Acompanhamento dos 75 itens da auditoria do commit `6a4c77de`. Cada item diz
o estado **medido** hoje, a evidência (commit, teste, corpus) e o que falta.
Nada aqui é marcado como feito sem teste que o demonstre; "parcial" diz o que
já passa e o que continua recusado.

Legenda: **feito** · **parcial** · **aberto** · **decisão** (depende de uma
definição de escopo do produto antes de implementar).

## Nativo AOT e runtime

| Item | Estado | Evidência / o que falta |
| --- | --- | --- |
| N01 cópia de strings | feito | `com_texto` empresta o texto sem clonar (`charAt`, `substringUnchecked`, `codeUnitAt`, `split`); `bench/desempenho/textos.dart` termina (antes: >300 s); corpus nativo sob GC stress. |
| N02 FileSystemWatcher | parcial | Linux: inotify como a VM (`crates/runtime/src/io_observador.rs`), o descritor entregue ao laço de eventos como soquete interno; `corpus/nativo/34`: criação, modificação, mudança de nome, subdiretório, filtro de eventos, arquivo só, remoção do próprio diretório (fim do fluxo) e caminho inexistente, iguais à VM, também com --gc-stress. Falta macOS (FSEvents) e Windows (`ReadDirectoryChangesW` na porta de conclusão): lá `isWatchSupported` é falso e `watch` lança `FileSystemException`. |
| N03 Multicast | feito | `joinMulticast`/`leaveMulticast` como a VM em cada sistema (Linux `MCAST_JOIN_GROUP`; macOS/Windows `ip_mreq`/`ipv6_mreq`); `corpus/nativo/31`: entrada, datagrama pelo laço de multicast, saída e os `OSError` iguais aos da VM. No runner, o harness compara com a VM do mesmo sistema. |
| N04 mensagens de controle | feito | `sendmsg`/`recvmsg` com `SCM_RIGHTS` no Unix; `toSocket`/`toRawDatagramSocket` lançam como na VM (que não os suporta); `corpus/nativo/33`. |
| N05 SynchronousSocket | feito | os 11 natives sobre o `TcpStream` bloqueante do Rust; `corpus/nativo/32`: dados, fim do fluxo (`null`), `available`, `shutdown`, `closeSync`, conexão recusada e resolução falha, iguais à VM. |
| N06 formatos TLS | feito | `crates/runtime/src/tls_formatos.rs`: PKCS#12 (MAC conferida, PBES2 e PBE legadas RC2/3DES, sacos aninhados, BER), PKCS#8 cifrado e PEM cifrado legado do OpenSSL; a ordem de decisão da VM (PEM, e PKCS#12 só sem linha de início; DER solto recusado), `ArgumentError` para chave ausente e os textos do BoringSSL nas `TlsException`. `crates/cli/tests/fixtures/tls_formatos.dart`: 26 casos positivos/negativos e três apertos de mão, iguais à VM (`io_regressao::formatos_de_chave_e_certificado_como_a_vm`). |
| N07 renegociação TLS | feito | contrato definido e testado contra o `openssl s_server` (`io_regressao::renegociacao_legada_e_recusada`): sem a opção, a recusa e o `TlsException` da VM; com ela, o pedido é recusado igual (o rustls não renegocia; a VM renegociaria) — documentado em NATIVO-PLANO e no patch. |
| N08 @Native de variável | feito | primitivo, ponteiro, struct e `@Array` (uma e várias dimensões, elemento primitivo/ponteiro/composto) como variável nativa, com `Native.addressOf`; `corpus/nativo/19` (`in6addr_loopback`/`in6addr_any` da libc como struct, vetor e matriz), igual à VM no AOT, no JIT e com --gc-stress. |
| N09 compostos em VarArgs | feito | structs na parte variádica pela regra do alvo (`abi_c::argumento_variadico`: igual ao fixo, exceto a HFA no arm64 da Apple, que vai como inteiros, como no clang); as peças não entram no tipo `ret (fixos, ...)`. `corpus/nativo/17`: par de `Int32` e par de `Double` num `snprintf`, iguais à VM no AOT, no JIT e com --gc-stress. |
| N10 closures tipadas | feito | corpo tipado + entrada uniforme, ABI conferida no cabeçalho da closure (`corpus/nativo/28`); `closures` 110 → 40 ms. |
| N11 especialização contextual | parcial | `sort` de `List<int>`, despacho de poucos alvos (P2) e `add` sem caixa; falta especializar consumidores genéricos do SDK pelo chamador. |
| N12 inlining com exceção | aberto | o inliner ainda aceita só funções que não lançam. |
| N13 provas de intervalo | parcial | comprimento/dados pelo cabeçalho fixo (sem chamada por acesso); falta retirar as conferências repetidas sob prova. |
| N14 listas escalares compactas | aberto | elementos continuam `TaggedValue` de 16 bytes. |
| N15 Windows sem MSVC | aberto | |
| N16 macOS sem CLT | aberto | |

## JIT

| Item | Estado | Evidência / o que falta |
| --- | --- | --- |
| J01 laço síncrono longo | aberto | |
| J02 recolher gerações | aberto | |
| J03 recarga estrutural | aberto | |
| J04 granularidade | aberto | |
| J05 depuração | decisão | |
| J06 spawnUri | feito | matriz AOT × JIT × VM em docs/JIT.md ("O que não executa"): `spawnUri` recusado nos dois perfis (um programa por processo; a VM AOT também só aceita snapshot AOT), com mensagem própria no JIT; `Platform.script` do JIT passou a ser o `.dart` (era o executável do `dartforge`), e o URI relativo resolve contra ele; `crates/cli/tests/jit_programa.rs`. |

## JavaScript

| Item | Estado | Evidência / o que falta |
| --- | --- | --- |
| JS01 publicação sem convergência | feito | sem ponto fixo, o bundle sai sem poda do código do usuário e com aviso; `DARTFORGE_JSPROD_ESTRITO=1` recusa. |
| JS02 resolução comum | parcial | identificadores de topo pela resolução comum; membros, `this` e extensões ainda pelo resolvedor do emissor. |
| JS03 instanciações genéricas | aberto | |
| JS04 quatro fixtures modernos | aberto | 347, 350, 351, 352 em `corpus/moderno/PENDENTES`. |
| JS05 despacho direto | aberto | |
| JS06 minificação | aberto | |
| JS07 cache de análise | aberto | |
| JS08 SDK pela trilha JS | decisão | |
| JS09 carregamento diferido | decisão | |

## Tipos e analisador

| Item | Estado | Evidência / o que falta |
| --- | --- | --- |
| T01–T08 | feito | catraca de inferência: 94/95 programas iguais ao oráculo; a divergência restante (`Null`/`Never?` em flu20) é deliberada e documentada. |
| A01 ampliar diagnósticos | parcial | Placar do corpus (`dartforge-paridade placar`, 23.030 registros do oráculo, medido em `f5080479` e depois de cada passo): **11.758 → 12.241 na posição exata (51,1% → 53,2%)**, mensagem igual 11.366 → 11.849, FP 1.212 → 1.086, FN 10.949 → 10.502, posição errada 323 → 287; nenhum código perdeu acerto. Por código (acertos/oráculo, FP): cláusulas de herança (`analise::clausulas`, porte de `_checkDirectSuperTypes` e da porta de `_checkClassInheritance`) — `subtype_of_disallowed_type` 0 → 94/94, `mixin_inherits_from_not_object` 0 → 94/94, `mixin_class_declares_constructor` 0 → 82/82, `mixin_class_declaration_extends_not_object` 0 → 22/22, `subtype_of_deferred_class` 0 → 7/7, `no_generative_constructors_in_superclass` 0 → 5/5, `concrete_class_has_enum_superinterface` 0 → 6/8, `mixin_super_class_constraint_deferred_class` 0 → 1/1, todos com 0 FP/posição/mensagem; privados não usados (`analise::privados`) — `unused_element` 88 → 165/279, `unused_field` 0 → 34/122, 0 FP; `dead_code` 16 → 74/255, posição errada 43 → 7, FP 36 → 3 (dois num arquivo em que o analyzer 3.6.2 quebra). Casos mínimos conferidos com o `dart analyze` 3.6.2 nos testes de `analise`/`types`. Em 40 pacotes do pub-cache (1.252 arquivos, oráculo 3.6.2) nenhum FP desses códigos. FP publicados do limitless_ui (V01): `// ignore` passou a valer para erro, como no `_filterIgnoredErrors` do analyzer, com o `cannot-ignore` das opções (B3, 124 FP), e os gerados do `build_runner` fora de `lib/` são vistos (B4, 69 FP); na cópia do projeto, `dartforge analyze` dá 0 publicados e 1 interno (`undefined_method` de `call` em `Function`, fora deste item). Candidatos a `verificados.txt` (0 erro emitido no corpus, falta conferir os projetos do proprietário): os dez códigos das cláusulas, `invalid_use_of_type_outside_library` e `unused_field`. Falta: os FN grandes de tipo (`invalid_override` 149, `wrong_type_parameter_variance_position` 132, `invalid_annotation` 108…) e os FP de parser (`expected_token` 317). |
| A02 mensagens específicas | parcial | Feito: `class_used_as_mixin` FP 91 → 0 (a porta do `ErrorVerifier`: supertipo proibido, cláusula adiada, erro de mixin ou superclasse só com `factory` desligam as verificações seguintes; `with Enum` em classe abstrata, alias de classe como mixin); `extends Enum`: `invalid_use_of_type_outside_library` FP 2 → 0 pela mesma porta, sem perder acerto (330); privados não usados (acima); posições de `dead_code` (trecho até o fim do bloco básico, ramo inalcançável inteiro, atualizações do `for`, construtor até o fim). Aberto: alias de `typedef` nas mensagens (161 mensagens diferentes de `type_argument_not_matching_bounds`: exige tipos com a anotação do alias em `types`, e o `Never?` sem normalizar para `Null`; o `subtype_of_disallowed_type` já exibe o alias pela forma escrita); `typedef` como supertipo além do que as cláusulas cobrem; `invalid_override` (149 FN, depende de `types`). |
| A03 augmentations restantes | parcial | Feito: `augment enum` (valores novos depois dos da declaração, membros, `with`/`implements`) em `elements` e `emit_js`; o `dart analyze` 3.13.4 aceita e tipa igual (teste `augmentation_de_enum_acrescenta_valores_e_membros`); nenhum CFE executa augmentation de enum (3.13.4: "already declared"), então não há oráculo de execução. Aberto, com diagnóstico de recusa: `extension` e `extension type` (o parser ainda exige `on` e a representação que a augmentation omite; o CFE 3.13.4 quebra), variável de topo e campo (o analyzer 3.13.4 dá `declaration_already_complete` para inicializador sobre variável completa; o CFE 3.13.4 quebra), enum com construtor primário. Ver docs/AUGMENTATIONS.md §2 e §4. |
| A04 escopo de imports por unidade | feito | `Library::escopos_de_unidade`: com imports em partes, cada unidade resolve nos próprios imports, depois nos do arquivo que a incluiu; prefixos idem; declarações da biblioteca antes de todo import. Usado por `types`, `elements` (supertipos), `emit_js` e `analise`. Antes o `compile-js` escolhia o import errado em silêncio; `corpus/macros/406_partes_imports_313` sai igual ao CFE 3.13.4 e sem diagnóstico, como no analyzer 3.13.4; 400–405 continuam iguais. Assinatura omitida na augmentation herdada da declaração aumentada (`augmentation_herda_tipos_omitidos`: os quatro erros do analyzer 3.13.4; sem a herança, nenhum). Limites: LSP, gerador ngdart, hospedeiro de macros e a lista de extensões aplicáveis ainda usam o escopo da biblioteca; função genérica e construtor não herdam tipos; o CFE 3.13.4 não herda (divergência registrada). |

## LSP

| Item | Estado | Evidência / o que falta |
| --- | --- | --- |
| L01 diagnósticos tipados | feito | publicados no fluxo contínuo com invalidação dos dependentes. |
| L02–L07 | aberto | |
| L08 formatação/pull | decisão | |

## Build e macros

| Item | Estado | Evidência / o que falta |
| --- | --- | --- |
| B01–B07 | aberto | |

## Ngdart

| Item | Estado | Evidência / o que falta |
| --- | --- | --- |
| NG01 providers | feito | todas as formas do corpus, inclusive filho com `providers:` no template (i72); recusas explícitas i76–i78. |
| NG02 ContentChild(ren) | feito | no componente e em filho com `read:` (d09, h01, i68, i69, i73). |
| NG03 vários componentes | feito | i24 (sem folha de estilo). |
| NG04 template/ngTemplateOutlet | feito | `<template #t>` e `*ngTemplateOutlet` (i30); `<template dir let-x [dirX]>` com diretiva estrutural (i84, i86); `@ViewChild` de `<template>` (i85, i87). Recusados com motivo: `<template>` com diretiva fora da forma exata, no conteúdo projetado, `@ViewChild` de `<template>` em lista/`*`. |
| NG05 consultas dinâmicas | parcial | qualquer profundidade (i97, j06), vários resultados em `*ngFor` (i96), componente filho por `#ref` e por tipo (j09, j10), `read: ElementRef`/`Element` e `ElementRef` sem `read:` (i98). Recusados: filho `onPush` em `*`, `read:` de outro token ou em consulta dinâmica, mistura com estáticos. |
| NG06 refs/exportAs | parcial | `#d="x"` com `exportAs` (i93), `#f="ngForm"` simples (j03). Recusados com motivo: `#ref` repetido/sombreado por `let` (i95, escopo por visão), `ngForm` com `ngControl` (i94, `@SkipSelf` do `NgControlName`), `@ViewChild` de `#ref` com valor. |
| NG07 HostBinding | feito | componente: propriedade, `attr.x` com segurança, `style.x[.unidade]`, sem argumento, herdado (i89–i91, j02); diretiva: `attr.x`, propriedade, sem argumento (j05). Recusados: `class`, `attr.x.if`, namespace, `style.x` final/de tipo desconhecido ou em diretiva. |
| NG08 i18n | feito | HTML dentro da mensagem (i99, j04), `@i18n` em `*` (j01). Recusados: tag com atributo/ligação dentro, entidade HTML, filho, handler de evento na mesma visão. |
| NG09 encapsulation | feito | `none` com `styleUrls` (i88, `.css.dart`), `styles: [..]` emulado e `none` (j07, j08), sem folha (b06). Recusados: `none` com Sass, `@import`, item não literal. |
| NG10 segurança | feito | `[attr.x]` com contexto de segurança e `[style]` (i92). |

Placar do corpus: 219 arquivos idênticos ao `build_runner` oficial, 5 recusados (i76–i78 e as sondas i94, i95).

## Validação e documentação

| Item | Estado | Evidência / o que falta |
| --- | --- | --- |
| V01 projetos do dono | aberto | projetos não disponíveis nesta máquina. |
| V02 matriz única | aberto | |
| V03 CI e pesados | parcial | Pesado de `040cb503`: nativo, gc-stress, moderno, JS e análise verdes; corrigidos o placar consolidado (sintaxe do PowerShell), o JIT × AOT no Windows (o `/OPT:REF` do `lld-link` ligava o ICF e fundia funções de corpo igual: `left == right` dava `true`) e o 17_ffi_varargs no Windows (`_snprintf` do `msvcrt.dll`). |
| V04 timeouts de benchmark | feito | `comparar-desempenho.py` com prazo, "esgotou" por executor, faixa mín–máx; núcleos separados (add/leitura/sort). |
| V05 semântica além de stdout | parcial | ordenação conferida por multiconjunto; erros de índice iguais aos da VM (`corpus/nativo/30`). Falta: exceção não capturada sai com 101 e `Uncaught exception: …`; a VM sai com 255 e `Unhandled exception:` + rastro (o 101 é o contrato atual do JIT e do AOT). |
| V06 documentação | feito | NATIVO, NATIVO-PLANO, JIT, ESTADO, PLANO e pesquisas conferidos com o código; afirmações velhas marcadas como Histórico. |
| V07 natives pendentes × APIs | feito | `docs/NATIVOS-PENDENTES.md`: dos 45 pendentes restantes, 8 são API pública ausente (FileSystemWatcher e heap snapshot). |
