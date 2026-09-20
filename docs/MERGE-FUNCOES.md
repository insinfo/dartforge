# Fusão opcional de funções idênticas

```dart
int soma(int n1, int n2) => n1 + n2;
int soma2(int n1, int n2) => n1 + n2;
void main() { print(soma(1, 2)); print(soma2(3, 4)); }
```

Use `dartforge compile entrada.dart saida.mjs --merge-identical-functions`.
A mesma flag é aceita por `emit-llvm` e `aot`, inclusive junto com `--optimize`.
Por padrão o passe está desligado. Nesse exemplo, a segunda definição desaparece
e suas chamadas passam a usar a primeira; a ordem dos argumentos e seus efeitos
são preservados. A função `main` continua presente e exportada no JavaScript.

## Contrato conservador

O passe opera após resolução e análise semântica. Compara chaves estruturais
completas, delimitadas por comprimento, sem confiar em igualdade de hash. Ignora
localizações de fonte, mas considera assinatura, tipos nominais, nomes de
parâmetros/locais, ordem de instruções, operadores, literais e destinos de chamadas,
incluindo a resolução de extensions. Nomes vindos de bibliotecas já estão ligados
e preservam a identidade de seus namespaces.

Escolhe deterministicamente a primeira representante segura. Uma função cujo
nome aparece como variável ou parâmetro em qualquer parte do programa não pode
ser representante, evitando capturar chamadas redirecionadas em um escopo local.
Reescreve chamadas diretas também dentro de métodos, campos e extensions, mas não
funde os próprios métodos nem `main`. Não renomeia parâmetros para reconhecer
equivalência, não faz prova algébrica, não resolve equivalência recursiva geral.
O subconjunto não suporta funções como valores/tear-offs; estender isso exigirá
rever identidade observável antes de ampliar o passe.

Nomes e localizações apresentados em stack traces podem corresponder à função
representante. Não há source maps completos. Essa consequência também faz parte
do caráter opt-in da transformação.

No JS, quando ambas as flags são usadas, constantes são avaliadas primeiro e a
fusão pode reconhecer novos corpos iguais. No LLVM, o passe de constantes JS
não é aplicado; `aot --optimize` seleciona otimização O2 no driver nativo.
As opções completas participam da chave do cache da sessão.

## Medição

`cargo bench -p dartforge-compiler --bench merge` produz JSON com 15 amostras de
10 compilações de um programa original de 200 funções idênticas, três aquecimentos
por configuração e contagem de bytes/funções emitidos. Mede o pipeline inteiro
em memória, sem iniciar um processo por compilação. Os modos rodam em sequência;
efeitos térmicos e de ordem são possíveis. É um cenário favorável à fusão, não
uma estimativa de ganho em aplicações reais nem comparação com DDC/dart2js.

A chave completa aumenta trabalho e memória transitória: esta opção é adequada
para experimentos de tamanho/compilação otimizada, não é habilitada silenciosamente
em desenvolvimento.
