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
