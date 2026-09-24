# Incremento 25 — sintaxe moderna posterior ao Dart 3.6.2

> **Correção (2026-09-23).** Este incremento é da trilha velha
> (`crates/syntax`/`crates/parser`) e o seu §4 está **errado**: `Tipo nome`
> num construtor primário é parâmetro **simples**, que não declara campo
> (spec aceita, `accepted/3.13/primary-constructors`, `:1000-1002`; oráculo
> 3.13.4: `class Nome(String cru)` não tem campo `cru`); só `var`/`final`
> declaram. `this.nome` não exige nada além do campo; a parte `this : … { }`,
> `new`/`factory` sem o nome da classe, `enum` e `extension type` também têm
> construtor primário. O §1 também deixava de fora `catch (_, _)` e
> parâmetros de tipo `<_>`, que a 3.7 aceita. O contrato vigente, conferido
> contra o SDK 3.13.4, é o de [`VERSOES-LINGUAGEM.md`](../VERSOES-LINGUAGEM.md)
> §4, implementado na trilha nova (`crates/frontend` → `emit_js`).

Dart 3.6.2 continua sendo o **mínimo** de compatibilidade do projeto, não um teto.
Este incremento implementa quatro recursos que entraram na linguagem depois desse
alvo. Cada um é identificado pela versão mínima em que existe, conforme a política
registrada no [PLANO](../../PLANO.md).

| Recurso | Versão mínima | Estado |
| --- | --- | --- |
| Variáveis e parâmetros curinga `_` | 3.7 | JavaScript |
| Elementos null-aware `?valor` em coleções | 3.8 | JavaScript |
| Atalhos de ponto `.membro` | 3.10 | JavaScript |
| Construtores primários `class C(...)` | 3.13 | JavaScript |

O backend LLVM/AOT rejeita elementos null-aware e atalhos de ponto com diagnóstico
explícito e span da expressão culpada. Curingas e construtores primários não criam
formas novas para o backend nativo: viram, respectivamente, ligações descartadas e
campos com construtor comum, portanto o AOT os aceita sem mudanças.

## 1. Curingas `_` (Dart 3.7)

`_` deixou de declarar um nome. Passam a ser aceitos, sem colisão:

```dart
var (id, _, email) = (102, 'Ignorado', 'usuario@dev.com');
var _ = calcular();
var _ = calcular();
lista.forEach((_) => print('ação'));
int comparar(int a, int _) => a;
```

Regras implementadas:

- Locais, parâmetros de funções, métodos e closures chamados `_` não entram no
  escopo. Podem repetir na mesma assinatura ou bloco.
- O inicializador continua sendo avaliado: `var _ = f();` executa `f()`.
- Ler `_` produz `Unknown identifier '_'`, a menos que outra declaração no escopo
  realmente se chame `_` (campos, membros e declarações de topo continuam nomes
  comuns, como na especificação).
- O cabeçalho de `for` aceita `for (var _ = 0; ...)` sem declarar a variável.

No JavaScript cada curinga recebe identidade única (`$df_$wildN`) porque a
linguagem alvo proíbe redeclarar um nome no mesmo escopo. Como a análise garante
que nenhuma referência alcança esses nomes, a renomeação não é observável.

**Limite:** parâmetros de tipo (`class C<_>`) e cláusulas `catch` ainda não
participam, porque genéricos de classe e `try/catch` não estão implementados.

## 2. Elementos null-aware `?valor` (Dart 3.8)

```dart
var cabecalhos = [
  'Content-Type: application/json',
  ?urlOpcional,   // some quando é null
  ?tokenOpcional,
];
var mapa = <String, int>{'a': 1, ?chave: 2, 'c': ?valor};
```

Contrato:

- Válido apenas **diretamente** dentro de um literal de lista ou nas duas posições
  de uma entrada de mapa. Em qualquer outra posição o diagnóstico é
  `Null-aware elements are only valid directly inside list or map literals`.
- O operando é avaliado **exatamente uma vez**, na ordem escrita.
- O tipo do elemento é o tipo do operando sem `null`; o contexto repassado ao
  operando é o tipo do elemento tornado anulável.
- Numa entrada de mapa, a entrada **inteira** desaparece quando qualquer posição
  marcada com `?` avalia para null.
- `?null` e operandos de tipo `Null`, `void` ou não inferido são rejeitados.
- `??valor` (aninhamento do prefixo) é rejeitado no parser.

**Limite:** o operador só existe em literais de lista e mapa. `Set` ainda não é
suportado pelo subconjunto e spreads (`...` e `...?`) continuam fora.

## 3. Atalhos de ponto `.membro` (Dart 3.10)

```dart
enum Alinhamento { esquerda, centro, direita }
void processar(Alinhamento a) => print(a.name);

processar(.centro);
Alinhamento x = .direita;
mostrar(.origem());   // fábrica nomeada
mostrar(.new(7));     // construtor sem nome
```

Contrato:

- O alvo vem **apenas** do tipo de contexto da posição: argumento de chamada,
  anotação de variável, tipo de retorno esperado ou tipo de elemento. Sem contexto
  o diagnóstico é `Dot shorthand requires a context type in this position`; o tipo
  nunca é inferido a partir do próprio atalho.
- Contexto anulável resolve a classe subjacente: `Cor? c = .vermelho;` é aceito.
- Formas aceitas: valor de enum (`.nome`), fábrica nomeada (`.nome(args)`) e
  construtor sem nome (`.new(args)`). `.new` é a única palavra reservada aceita
  como membro e exige lista de argumentos.
- O tree shaking preserva a classe alvo consultando a resolução; sem isso a classe
  seria podada por não aparecer nominalmente no código.

**Limite:** membros estáticos e constantes de classe ainda não existem no
subconjunto, portanto `.constante` só resolve valores de enum.

## 4. Construtores primários (Dart 3.13)

```dart
class Cliente(String nome, String email);

class Contador(var int valor) {
  int proximo() { valor = valor + 1; return valor; }
}

class Legado(this.codigo) {
  final int codigo;
  int dobro() => codigo * 2;
}
```

Contrato:

- `Tipo nome` e `final Tipo nome` declaram campo **final**; `var Tipo nome`
  declara campo mutável. A escolha de `final` como padrão segue a proposta
  oficial de *declaring constructors*.
- Os campos declarados pela lista primária precedem os do corpo, preservando a
  ordem de inicialização escrita na declaração.
- `this.nome` exige que o campo exista no corpo da classe, sem inicializador; é
  de lá que vem o tipo. Sem o campo, o diagnóstico é
  `a this. primary parameter requires the field declared in the class body`.
  Esta é a diferença deliberada em relação a exemplos informais que escrevem
  `class Cliente(this.nome, this.email);` sem declarar campo algum: essa forma
  não tem tipo e é rejeitada.
- Redeclarar no corpo um campo da lista primária é erro.
- A classe não pode declarar outro construtor sem nome.
- O corpo pode ser `;` (vazio) ou `{ ... }` com membros adicionais.
- Só vale para `class`; `mixin` e `enum` rejeitam a lista primária.

## Correção incidental

Fábricas nomeadas passaram a aceitar corpo em bloco, não apenas corpo de
expressão (`factory C.nome(...) { ... }`), equiparando-as às demais funções.

## Testes

- `tests/conformance/modules/modern24/` — fixture ponta a ponta com os quatro
  recursos, executada em `crates/compiler/tests/modern24.rs` nas combinações de
  otimização e tree shaking.
- `crates/compiler/tests/modern24.rs` — contratos positivos e cada diagnóstico
  negativo, incluindo avaliação única do operando `?` e ausência de referência a
  curingas no JavaScript emitido.

## Oráculo diferencial pendente

O SDK instalado nesta máquina é o Dart 3.6.2, que **não aceita** nenhuma das
quatro sintaxes. `main.stdout` foi conferido executando o JavaScript emitido em
Node.js, o que não é um oráculo independente. A comparação diferencial exige
instalar um SDK que implemente os quatro recursos; até lá este módulo não
participa da suíte de conformidade contra o SDK oficial.

## Próximos passos

- Parâmetros nomeados (`{...}`), `required` e valores padrão, pré-requisito dos
  parâmetros nomeados privados `this._campo` do Dart 3.12.
- `Set`, spreads e `?...` em coleções.
- Membros estáticos e constantes de classe, ampliando o alcance dos atalhos de
  ponto.
- Extension types (Dart 3.3) com apagamento estático distinto de classes.
