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
| N02 FileSystemWatcher | aberto | natives `FileSystemWatcher_*` pendentes. |
| N03 Multicast | feito | `joinMulticast`/`leaveMulticast` como a VM em cada sistema (Linux `MCAST_JOIN_GROUP`; macOS/Windows `ip_mreq`/`ipv6_mreq`); `corpus/nativo/31`: entrada, datagrama pelo laço de multicast, saída e os `OSError` iguais aos da VM. No runner, o harness compara com a VM do mesmo sistema. |
| N04 mensagens de controle | feito | `sendmsg`/`recvmsg` com `SCM_RIGHTS` no Unix; `toSocket`/`toRawDatagramSocket` lançam como na VM (que não os suporta); `corpus/nativo/33`. |
| N05 SynchronousSocket | feito | os 11 natives sobre o `TcpStream` bloqueante do Rust; `corpus/nativo/32`: dados, fim do fluxo (`null`), `available`, `shutdown`, `closeSync`, conexão recusada e resolução falha, iguais à VM. |
| N06 formatos TLS | aberto | PKCS#12 e chave criptografada recusados. |
| N07 renegociação TLS | aberto | contrato da opção a definir e testar. |
| N08 @Native de variável | parcial | variáveis de tipo primitivo e ponteiro lidas e gravadas (`corpus/nativo/19`); struct/array como variável nativa continua recusado. |
| N09 compostos em VarArgs | aberto | recusado no lowering. |
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
| J06 spawnUri | aberto | |

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
| B01–B07 | aberto | |

## Ngdart

| Item | Estado | Evidência / o que falta |
| --- | --- | --- |
| NG01 providers | feito | todas as formas do corpus, inclusive filho com `providers:` no template (i72); recusas explícitas i76–i78. |
| NG02 ContentChild(ren) | feito | no componente e em filho com `read:` (d09, h01, i68, i69, i73). |
| NG03 vários componentes | feito | i24 (sem folha de estilo). |
| NG04 template/ngTemplateOutlet | parcial | `<template #t>` e `*ngTemplateOutlet` (i30); `<template>` com diretiva (i84) e `@ViewChild` de template (i85) recusados. |
| NG05–NG08, NG10 | aberto | |
| NG09 encapsulation | parcial | `emulated`/`none` sem folha de estilo (b06); com estilos, recusado. |

Placar do corpus: 192 arquivos idênticos ao `build_runner` oficial, 5 recusados.

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
