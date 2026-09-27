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
| A01 ampliar diagnósticos | parcial | 51,1% na posição exata; trabalho por código. |
| A02 mensagens específicas | aberto | |
| A03 augmentations restantes | aberto | |
| A04 escopo de imports por unidade | aberto | |

## LSP

| Item | Estado | Evidência / o que falta |
| --- | --- | --- |
| L01 diagnósticos tipados | feito | publicados no fluxo contínuo com invalidação dos dependentes. |
| L02–L07 | aberto | |
| L08 formatação/pull | decisão | |

## Build e macros

| Item | Estado | Evidência / o que falta |
| --- | --- | --- |
| B01 executor nativo de builders | aberto | O executor de builders pela VM funciona (`corpus/builders`: 57/0/0). Medido o caminho nativo (2026-09-27): `dartforge compile-native` do bootstrap do `pacotes/build_executor` (com `build_resolvers`/`analyzer`) no `cadeia_configuracao` para em **222 construções não suportadas** pelo backend nativo (177 chamadas, 27 instanciações não resolvidas, testes de tipo genérico `T`, padrões constantes), 61 delas no `resolver.dart` do analyzer. Depende do alvo governante "compilar e executar o `package:analyzer`" (PLANO.md); o mesmo caminho das macros (B06) serve quando o backend chegar lá. |
| B02 pós-processadores | feito | Âncoras do `_addPostBuildPhaseAnchors` com as `inputExtensions` do objeto (`build.entradas_pos`), execução pelo `runPostProcessBuilder` (`build.posprocessar`), saídas no registro da âncora; `deletePrimaryInput` tira a saída gerada da geração publicada (o `FinalizedReader`), e numa fonte avisa/recusa no `--estrito` (sem `build -o` nem `serve` que a esconda). `corpus/builders` pela VM 57/0/0 (antes 56/1/0), `pos_processador_pela_vm`, `ancora_de_pos_processador_escreve_so_asset_novo`. `BUILD-MOTOR.md` §6.1. |
| B03 ngdart por declaração e dependência | feito | Edição de `.dart` regenera o arquivo e o fecho de importadores (reindexados): 219 edições no `corpus/ngdart`, incremental = do zero em todas, 209 sem regenerar o pacote (`ng_edicao_dart_em_cada_arquivo_igual_ao_do_zero`; corpo, `@Input` em filho, seletor e import em `ng_incremental_igual_ao_do_zero`). Dependências com componentes geradas pelo nativo com rodada própria e consultas só do que alcançam: `corpus/ngdart_dependente` (oráculo do `build_runner`) 5/5 iguais e incremental = do zero; 219 templates de `ngdart`/`ngcompiler` iguais na medição. Volta ao estágio A: arquivo novo/apagado, parte, recusa. |
| B04 cache persistente e concorrência | feito (opcional) | `persistencia.rs`: estado em `.dart_tool/dartforge/build/estado` só a pedido (`--estado`, `DARTFORGE_BUILD_ESTADO=1`; regra governante 6). Chave global (motor, executável, plano, lock, `--release`), código do builder Dart pelo depfile; revalidação completa na primeira verificação, saída `source` conferida no disco. `estado_salvo_entre_processos` (entrada, opção, código do builder, saída apagada, estado corrompido) e `estado_salvo_pela_vm` (19,9 s → 0,27 s, 0 ações). **Decisão**: ações Dart seguem em série (um processo, um `Resolvers`, canal de uma ação por vez); nativo e apoio em paralelo. |
| B05 Sass e mapas | parcial | Oráculo por forma e opção (`crates/gerador_ng/tests/sass_formas`, 73 formas do `sass_builder` real, `scripts/sass-formas.sh`): `compressed` 57 iguais/16 recusadas, `expanded` (novo) 54/19, **0 diferentes**; corrigidas onze formas em que o `compressed` publicado divergia. Falta: `.css.map` (recusado com motivo; o `dev` o liga por padrão, então lá o `.css` vem do executor Dart/apoio); o caminho do shim do ngdart mantém as normalizações do `compressed` (registrado em `sass.rs`). |
| B06 executor nativo de macros (D4) | feito | `macros_host::nativo`: bootstrap compilado pelo `compile-native` do DartForge, sem VM; `dartforge macros --nativo`, `DARTFORGE_MACROS_NATIVO` no `compile-js`. `executor_nativo_bate_com_o_cfe`: casos 410–413 iguais ao CFE 3.6.2 byte a byte; sessão gravada do 410 (98 mensagens) igual em 87 ms (VM 1011 ms). Falta tornar o padrão da distribuição (exige o `dartforge` com `nativo` e o Clang) e ligar ao `dev`. |
| B07 macros: erro e formas | parcial | `erro_na_macro_preserva_a_ultima_geracao_valida` e `materializado_nas_duas_formas_nao_duplica` (formas 3.6 e atual). Falta cobertura de introspecção além dos quatro casos do corpus e o ciclo no `dev` (que ainda não aplica macros). |

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
