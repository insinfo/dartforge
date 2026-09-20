# Extensions com despacho estático

O parser aceita extensions nomeadas sobre int, String, bool e classes não anuláveis:

```dart
extension IntMath on int {
  int triple() { return this * 3; }
}
void main() { print((5).triple()); }
```

O analisador escolhe o método pelo tipo estático ou promovido do receptor. Métodos de
instância têm prioridade. Um campo existente também impede tratar aquela chamada como
extension. Entre extensions aplicáveis, uma classe mais específica prevalece; empate
produz diagnóstico. A classe concreta de um objeto em execução não troca a extension
selecionada para uma variável ou parâmetro de tipo base.

semantic::analyze retorna uma tabela de alvos identificados pelo intervalo da chamada.
A HIR transporta essa tabela até a emissão. Cada método vira uma função independente,
chamada com receptor e argumentos avaliados uma vez e na ordem original. Em ESM estrito,
o receptor primitivo mantém seu tipo e é acessível como this dentro da função.

A otimização de `null ?? chamada` preserva o intervalo da expressão selecionada para
manter sua associação com o alvo resolvido. Testes executam as formas direta e otimizada,
incluindo operadores aninhados, efeitos, falhas de `!` e despacho sobre classes base.

## Limites atuais

- Apenas uma unidade sem imports. Extensions em grafos de bibliotecas são rejeitadas
  até que visibilidade, imports e precedência dessas declarações estejam implementados.
- Extensions nomeadas, métodos de instância tipados e parâmetros posicionais obrigatórios.
- Sem extensions anônimas/genéricas, getters/setters, membros static ou invocação explícita
  `NomeDaExtension(receptor).metodo()`.
- O tipo declarado após on não pode ser anulável neste incremento; um receptor anulável
  exige promoção válida ou asserção `!`.
- Não substitui contratos de Object nem adiciona métodos a protótipos JavaScript.
- A tabela de spans ainda não é uma IR completa com IDs de todos os símbolos/expressões.
