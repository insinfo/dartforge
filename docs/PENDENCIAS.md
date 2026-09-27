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
| JS02 resolução comum | parcial | todo identificador (lido, chamado, escrito, alvo composto, receptor estático `C.m`/`p.x`/`Ext(x).m`) usa `alvo_do_identificador`, que traduz o `Resolved` comum: local/parâmetro, parâmetro de tipo, prefixo, topo, membro estático e constante de enum, membro por `this` implícito (com a substituição do dono), membro de extensão, campo estático de extensão. No `corpus/js`: 19.662 alvos pela resolução comum, 28 pela busca própria (`this` de extensão, campo de record por `this`, `dynamic`/`Never` como valor, nomes que a comum não registra); JS dos 196 programas idêntico byte a byte com e sem a tradução. Falta: acesso com receptor explícito (`a.b`, `a.m()`) ainda escolhe o membro por `lookup_member` do emissor sobre o tipo (agora o comum), não pelo `Resolved` do nó; identificador em padrão (sem `ExprId`); `this` de extensão. `docs/INFERENCIA-JS-ALINHAMENTO.md` §9. |
| JS03 instanciações genéricas | parcial | `crates/types` registra a instanciação de cada chamada genérica (`UnitBodyTypes::instanciacoes`); o emissor a usa em funções, métodos, extensões e construtores (805 de 805 chamadas do `corpus/js`, 0 pela dedução própria), e tira do tipo comum literais de coleção, closures (parâmetros/retorno sem anotação), leitura de locais promovidos, refinamento de qualquer nó por subtipo próprio e o tipo cru (`dynamic` para limite implícito). Conferência: 336 → 18 expressões com tipo divergente (as 18 são de representação ou erro dos dois lados). Teste `p10_tipos_reificados` (`is`/`as`/`runtimeType`/`T` impresso de chamadas, literais, construtores, closures): 12 linhas divergiam da VM, agora nenhuma. Falta retirar a dedução própria (`emit_args_infer` com parâmetros livres, `infer_elements_ty`, retorno pelos `return`), que ficou como recuo quando o dado comum falta ou menciona parâmetro fora de escopo — sem medir nos projetos reais (lacunas de inferência) a retirada trocaria tipo por `dynamic`. |
| JS04 quatro fixtures modernos | aberto | 347, 350, 351, 352 em `corpus/moderno/PENDENTES`. |
| JS05 despacho direto | parcial | com os tipos comuns (JS03) e a classe chamável despachada como `o.call(…)`, as chamadas genéricas do DDC no JS do `corpus/js` caíram de 214 para 198; as restantes têm receptor `dynamic`/`Function` pela especificação. No mundo, receptor `X & B` restringe pelo cone de `B`. A troca de `dsend` por chamada direta com alvo único não foi feita: o `dsend` do DDC confere o tipo de cada argumento (`_argumentErrors`, `…[_as](passedValue)`) e o método chamado direto não; exige argumentos provados atribuíveis e receptor provado da classe (`docs/JS-PRODUCAO.md` §2). |
| JS06 minificação | parcial | `minificar.rs`, ligada por padrão no `dartforge-jsprod`: comentários e espaço fora, quebras de linha e literais intactos, nenhum nome trocado. `01_print`: −12,4% bruto, −7,0% gzip −9 (runtime inteiro: −11,6% / −6,2%); runtime inteiro compactado passa no `node --check` e executa; corpus/js e moderno sem regressão; `determinismo --producao` idêntico com 1/4/8. Falta a renomeação de locais e propriedades, que exige analisador de JS com escopos sobre o `dart_sdk.js` (`docs/JS-PRODUCAO.md` §3). |
| JS07 cache de análise | parcial | índice do `dart_sdk.js` em `<cache>/jsprod-sdk-<chave>.bin` (`cache.rs`): chave pelo formato, texto inteiro do runtime, granularidade e código da classificação; conferência de chave, tamanho e segundo hash ao ler; escrita atômica; no máximo 4 no diretório. `01_print`: 455 → 130 ms por compilação, bundle byte a byte igual; testes de reuso, runtime/configuração incompatíveis, arquivo corrompido e descarte. Falta o resumo por biblioteca do usuário (§1.6, aplicação 2). |
| JS08 SDK pela trilha JS | decisão | fora do escopo desta rodada. Compilar `dart:core` e companhia pelo nosso front-end é o único caminho para o piso de ~1,5 MB do runtime do DDC (`docs/JS-PRODUCAO.md` §1.5), mas troca o runtime que hoje dá semântica exata de graça (`docs/EMISSAO-DDC.md`) por um que teria de reproduzir `dart:_rti`, interceptores e o protocolo do DDC, com o diferencial inteiro como critério; a fusão semântica de corpos (§3.5) depende de identidades (`DefinitionId`/`TypeId`/`StorageId`/`BodyId`) que o modelo ainda não separa. Decisão do proprietário: se e quando trocar o runtime. |
| JS09 carregamento diferido | decisão | fora do escopo desta rodada. Hoje `deferred` carrega tudo no arquivo único e `loadLibrary()` devolve um `Future` resolvido (semântica correta, sem divisão). Dividir em partes exige definir unidades de carregamento (o `ImportSet` do dart2js), o contrato de inicialização entre partes (bibliotecas, constantes e regras rti registradas depois do SDK podado) e o comportamento de erro de carga — antes de implementar (`docs/JS-PRODUCAO.md` §4). |

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
