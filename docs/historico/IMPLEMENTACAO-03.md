# Terceiro incremento — controle de fluxo, strings e Rustdoc

Alvo mantido: Dart 3.6.2. Implementação e revisão distribuídas entre três subagentes,
com integração e comparação usando o SDK local da versão exigida.

## Implementação

- Laços while, do/while e for clássico, break e continue sem rótulos.
- Atualizações de identificadores: ++, --, +=, -= e *= como instruções.
- Escopos do inicializador, corpo e atualização do for preservados na emissão JavaScript.
- Strings raw e escapes Dart, incluindo Unicode e pares surrogate válidos.
- Strings simples continuam emprestando o texto de entrada; somente escapes exigem decodificação.
- Strings emitidas por serialização JSON, com testes de controles e tentativa de injeção.
- Correção do sombreamento de parâmetros por variáveis locais mediante bloco léxico interno.
- Rejeição semântica de representações inválidas de cabeçalho for antes da emissão.
- Documentação de módulos e funções em português, exemplos executáveis e política de contribuição.
- CI gera Rustdoc incluindo itens privados com avisos tratados como erros.

Surrogates isolados, interpolação, strings multilinha e aspas triplas continuam rejeitados.
A análise de retorno permanece conservadora para laços. A implementação ainda não compila
aplicações Dart/ngdart completas e não demonstra vantagem de desempenho sobre DDC/dart2js.

## Verificação

- Formatação, Clippy com avisos como erros, build release e Rustdoc passaram.
- 72 testes Rust passaram, incluindo execução Node e exemplos da documentação.
- 13 casos diferenciais passaram contra Dart 3.6.2 com dart2js -O2.
- Relatório: [conformance-increment-03.json](dados/conformance-increment-03.json), com targetVersionMatched=true.

O CI executa a suíte Rust em Windows e Linux, além da comparação Dart em Windows.
Este relatório registra a validação local; os resultados remotos ficam no GitHub Actions.
