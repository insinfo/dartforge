# Metaprogramação: fases, dependências e desempenho

Metaprogramação é prioridade do DartForge. A primeira aplicação concreta é
JsonCodable incorporada em Rust; a evolução deve permitir macros de usuário sem
comprometer a edição incremental, a correção dos diagnósticos ou a previsibilidade.

## Referências e critérios

O [relato oficial de janeiro de 2025](https://dart.dev/blog/an-update-on-dart-macros-data-serialization)
aponta custo de introspecção semântica profunda e regressões na edição e na
compilação incremental. Ele não estabelece impossibilidade de metaprogramação,
nem comprova que mudar a linguagem de implementação resolva esses custos.
O objetivo do DartForge é reduzir consultas, materializações e invalidações,
medindo cada etapa. Afirmações de “100 vezes mais rápido” de ferramentas externas
não servem como benchmark deste compilador.

O protótipo local de macros fornece exemplos de fases, consultas e geração.
Sua API experimental não é tratada como especificação estável do Dart 3.6.2.
Dart 3.6.2 permanece baseline mínimo; funcionalidades posteriores são autorizadas.

## Contrato de fases

1. Types: estabelecer os tipos que uma macro pode criar. JsonCodable não cria
   tipos, portanto essa fase é explicitamente vazia para a macro incorporada.
2. Declarations: validar os alvos e reservar assinaturas de todos os membros
   gerados antes de produzir seus corpos.
3. Definitions: materializar os corpos usando os IDs e spans da compilação atual.

As barreiras são globais na unidade expandida. Uma falha não publica uma AST
parcial. Em um grafo de imports, a resolução semântica ocorre depois das expansões
locais. Introspecção profunda entre bibliotecas e dependências cíclicas de macros
exigirão um agendador próprio; não são resolvidas por adicionar nomes de fases.

## Cache de planos

Planos devem ser owned e limitados em memória. Não podem reter referências à
fonte, AST emprestada, IDs de classes/tipos ou intervalos da compilação anterior.
O nome, tipo e ordem dos campos e a política do construtor fazem parte do contrato.
Colisões de membros e anotações duplicadas precisam continuar sendo verificadas.

Alterar o corpo de um método independente não muda o plano JsonCodable; alterar
campos ou o construtor pode invalidá-lo. A AST e sua proveniência são sempre
materializadas para o contexto atual. Acerto de plano não significa que parsing,
análise semântica ou emissão JavaScript deixaram de executar.

O cache completo de JavaScript tem estatísticas separadas. Orçamentos precisam
limitar entradas e payload, permitir desativação e distinguir payload retido de
RSS do processo. Hits, misses, expulsões e nós materializados devem ser observáveis.

O plano atual é pequeno. Sua consulta pode custar mais que reconstruí-lo em
algumas entradas; benchmarks frios/aquecidos e com edições precisam registrar
isso. Nenhum ganho sobre DDC, dart2js ou hot reload é presumido.

## Hospedagem e augmentations

A macro incorporada é código Rust confiável do compilador e roda em processo,
sem RPC. Macros arbitrárias de usuário exigirão hospedagem isolada, protocolo
versionado, orçamento de execução, diagnóstico de falhas e rastreamento das
consultas que determinam a invalidação. Esses componentes permanecem pendentes.

A expansão atual modifica uma AST transitória em memória. Não escreve `.g.dart`
nem modifica a fonte do usuário. A sintaxe geral `augment`, o protocolo original
Dart macro_service e a execução de classes Dart de macro ainda não estão suportados.

Próximos marcos: consultas declarativas com dependências registradas, registro de
macros por URI/nome, introspecção entre bibliotecas, trabalhador isolado, orçamento
de recursos e suporte a macros escritas em Dart. O benchmark deve separar tempo
de transporte, execução da macro, análise da expansão e recompilação da aplicação.

## Reflexão estática: o que herdar da proposta de macros

A proposta de macros do Dart é o melhor modelo disponível para **reflexão gerada
em compilação**, e vale separar o que dela serve do que não serve.

O que serve é o modelo de fases e as regras de ordenação. O que não serve é a
arquitetura de hospedagem — e ela é justamente a parte que existe como código no
protótipo clonado em `references/dart-macros`.

### A hospedagem é o custo, e nós não precisamos dela

O protótipo não é a especificação: é `_macro_host`, `_macro_client`,
`_macro_server` e `macro_service`, com `handshake.g.dart`, `message_grouper.dart`
para enquadrar mensagens e 1.949 linhas de schema JSON de protocolo. Macro de
usuário é programa Dart completo, executado em isolate ou processo separado, que
introspecta o programa **por RPC** durante a compilação.

Esse é o custo que o relato de janeiro de 2025 aponta, e ele colide de frente com
o objetivo número um do DartForge. Uma consulta de introspecção que atravessa
serialização, socket e desserialização não cabe no orçamento de compilação
incremental que se quer defender contra o DDC.

A reflexão que precisamos não exige nada disso. Ninguém precisa escrever a macro
de reflexão: ela é sempre a mesma. Então ela é código Rust do compilador, em
processo, como a JsonCodable incorporada já é — **herdando o contrato de fases e
descartando o transporte**.

### A assimetria que torna reflexão mais difícil que serialização

`@JsonCodable` numa classe gera código *para aquela classe*. Reflexão recebe
perguntas *por nome, em execução*: `invoke('foo')`, onde `'foo'` pode ser
calculado. O gerador sabe sobre quais declarações emitir descritores; não sabe
quais nomes o programa vai pedir.

Daí a consequência que não dá para esconder atrás de API boa: **o conjunto
reflexível tem de ser declarado, não inferido**. É por isso que
`package:reflectable` exige capacidades explícitas na anotação em vez de
reflexão total. Não é limitação de implementação; é o preço de conviver com
tremor de árvore.

### Três regras da especificação que são ganho de incrementalidade

1. **Ordem lexicográfica, não ordem de fonte**, nos resultados de introspecção. A
   especificação diz isso explicitamente e dá o motivo: reordenar membros não
   deve obrigar a regerar. É propriedade de invalidação de cache, exatamente a
   nossa preocupação — a chave do plano ordena os campos, não os copia na ordem
   escrita.
2. **`OmittedTypeAnnotation` e inferência só na fase 3.** Um descritor precisa do
   tipo do campo, e `final x = f();` não tem tipo escrito. A regra da
   especificação — pode-se *emitir* o tipo omitido como token opaco antes, mas só
   se pode *ler* o tipo inferido na fase 3 — determina onde o texto de tipo do
   descritor é materializado. Antes disso ele não existe.
3. **É erro adicionar declaração que sombreie identificador já resolvido.** Isso
   evita re-resolução. Para nós: nomes de descritor vivem em namespace reservado
   que não pode sombrear nada.

A regra de aciclicidade (macro não se aplica na biblioteca onde é definida) sai
de graça, porque o gerador é do compilador e não da biblioteca compilada.

### A regra de equivalência semântica decide a forma da anotação

Se `@Reflectable` fosse anotação exclusiva do DartForge, o mesmo fonte deixaria
de **compilar** no dart2js. Isso viola a regra de equivalência semântica na forma
mais dura possível: não é comportamento diferente, é build quebrado.

A saída é a mesma do desenho de isolates: a anotação vem de pacote pub normal que
**também** traz um builder `build_runner`. Sob dart2js e DDC, `build_runner` gera
os descritores como parte; sob DartForge, o compilador gera a mesma API
nativamente, em processo, sem etapa de build. Mesmo programa, mesma semântica —
um dos dois só constrói mais rápido.

`package:reflectable` já tem exatamente essa forma. Então o alvo é a API dele, e
não uma inventada aqui: compatibilidade vem de mirar o que existe.

### O que continua impossível, e o diagnóstico que isso exige

Reflexão estática não é `dart:mirrors`. Mirrors responde sobre código que ninguém
declarou reflexível, e qualquer sistema dirigido por anotação é fechado por
construção. `MirrorSystem.findLibrary` e refletir objeto arbitrário não anotado
não podem funcionar junto com tremor de árvore — não é questão de esforço.

Logo `import 'dart:mirrors'` recebe diagnóstico próprio, que nomeia o substituto
em vez de só recusar.
