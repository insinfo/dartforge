# Pesquisa aplicada — desempenho, memória e deduplicação

Levantamento trazido pelo proprietário, organizado pelo que se aplica ao
DartForge. Serve de leitura obrigatória para quem for mexer no perfil de
produção do backend JavaScript e na arquitetura de memória do compilador.

A tese central, que vale repetir antes de qualquer otimização:

> Compartilhar o texto, a árvore sintática ou a implementação de uma função
> **não é** a mesma coisa que transformar duas bibliotecas Dart em uma só.

Essa distinção é o que permite otimizar sem quebrar identidade de tipos,
estado global e privacidade de nomes.

---

## 1. Trabalhos a estudar, e o que tirar de cada um

| trabalho | o que aproveitar |
|---|---|
| **Liška, _Optimizing large applications_ (2014), cap. 5 "Semantic Function Equality"** | achar funções equivalentes por estrutura, atributos e dependências; agrupar candidatos **antes** da comparação detalhada |
| **Bacon, _Fast and Effective Optimization of Statically Typed OO Languages_ (Berkeley, 1997)** | RTA: cruzar classes **instanciadas** com métodos alcançáveis para podar código e resolver chamadas virtuais |
| **Tallam et al., _Safe ICF_ (2010)** | quando duas implementações iguais podem compartilhar código sem destruir identidades observáveis |
| **Rocha et al., _Function Merging by Sequence Alignment_ (CGO 2019)** | fundir funções **parecidas**, não só idênticas |
| **Rocha et al., _HyFM: Function Merging for Free_ (LCTES 2021)** | desistir cedo de fusões que não compensam, em vez de construir a função fundida para só então medir |
| **Johnson, Amini, Li, _ThinLTO_ (CGO 2017)** | análise global por **resumos**, sem carregar a IR inteira na memória |
| **Mokhov, Mitchell, Peyton Jones, _Build Systems à la Carte_ (ICFP 2018)** | separar o **agendamento** da decisão de **reconstruir** — base do nosso substituto do build_runner |
| **Braun et al., _Simple and Efficient Construction of SSA_ (CC 2013)** | construir SSA direto, sem as etapas intermediárias clássicas |
| **Filliâtre, Conchon, _Type-Safe Modular Hash-Consing_ (ML 2006)** | compartilhar estruturas imutáveis equivalentes (tipos, assinaturas, listas) |
| **Sampson, _Flattening ASTs_ (Cornell)** | árvores em vetores com índices, em Rust — o que já fazemos e vale medir |

Ordem sugerida de leitura: Flattening ASTs, Liška cap. 5, o capítulo de RTA
do Bacon, ThinLTO.

---

## 2. Memória: o que já temos e o que falta

O projeto já segue boa parte disto — arenas, `SymbolId` com nicho
`NonZeroU32`, `Box<[T]>` de capacidade exata, testes de platô com o
`CountingAllocator` do `crates/instrument`. O que a pesquisa acrescenta:

1. **Separar dados por tempo de vida, não só por fase.** No daemon
   (`dartforge serve`) há três grupos: o estável (fontes de dependências,
   resumos), o da revisão corrente (árvores e resultados semânticos) e o
   temporário de uma análise. **Regra: nenhum cache cresce sem política de
   descarte explícita.** Uma arena global que nunca é liberada acumula
   todas as versões do projeto — problema de retenção, mesmo sem GC.

2. **Hash-consing com igualdade correta.** `List<String>` pode
   compartilhar representação; dois `Usuario` de bibliotecas diferentes
   **não** são o mesmo tipo só porque nome e campos coincidem.

3. **Menos conversões, não só parser rápido.** Nada de
   `fonte → AST → texto → outro parser → outra AST`. É exatamente o que o
   gerador do ngdart evita ao emitir `.template.dart` em memória — e o que
   ele vai evitar mais ainda quando emitir HIR direto.

4. **Medir `size_of` das estruturas**, não só contar nós.

---

## 3. Pacotes vendorizados: quatro deduplicações diferentes

Distinção que evita um erro caro:

| nível | o que pode ser compartilhado |
|---|---|
| conteúdo do arquivo (`SourceBlobId`) | armazenamento, por hash do conteúdo |
| árvore sintática (`SyntaxTreeId`) | o *parse*, se a versão de linguagem e o modo forem os mesmos |
| instância de biblioteca (`LibraryInstanceId`) | **nada**: identidade de tipos, estado global e privacidade são por biblioteca |
| corpo executável (`CodeBodyId`) | a implementação, se houver prova de equivalência |

O contraexemplo que fecha a questão:

```dart
import 'vendor_a/token.dart' as a;
import 'vendor_b/token.dart' as b;
// arquivos byte a byte iguais, com `int contador = 0;`

a.proximo();                         // 1
b.proximo();                         // 1  — contadores separados
(a.Token() as Object) is b.Token;    // false — tipos distintos
```

Unificar as bibliotecas mudaria o programa. **Compartilhar representação
interna, sim; unificar identidade, não.**

---

## 4. Deduplicação de funções: como fazer

1. **Candidatos baratos primeiro.** Agrupar por assinatura, estrutura de
   controle e sequência de operações normalizada. Com 100 mil funções, a
   comparação de todos contra todos seria ~5 bilhões de pares.

2. **Normalizar nomes locais, nunca identidades externas.**
   `f(int a) => a + 1` e `g(int b) => b + 1` são candidatas;
   `proximo() => ++contador` só é equivalente a outra se o `contador`
   resolver para o **mesmo** armazenamento.

3. **Refinar por dependências, até estabilizar** (refinamento de partição,
   como no ICF do `gold`/`mold`): duas funções idênticas que chamam
   símbolos diferentes só são equivalentes se os alvos também forem. Ciclos
   de chamada exigem tratar componentes fortemente conexos — hash recursivo
   ingênuo não resolve recursão mútua.

4. **Hash igual é candidato, não prova.** Depois do hash, comparação
   estrutural completa.

5. **Preservar identidade observável.** Se a referência da função escapa
   para comparação (`identical`, `==`), mantenha declarações separadas com
   corpo compartilhado — é a lição do Safe ICF. Em desenvolvimento,
   previsibilidade fonte↔saída vale mais que bytes; fusão agressiva é de
   produção.

---

## 5. Eliminação de código: alcançabilidade e RTA

"Duas implementações fazem a mesma coisa?" e "esta implementação pode ser
executada?" são perguntas diferentes.

RTA: cruzar métodos alcançáveis com classes **efetivamente instanciadas**,
iterando até o ponto fixo. No nosso caso, com ngdart, o grafo precisa
enxergar o que o codegen produz — fábricas, registros de injeção,
callbacks — ou receber resumos equivalentes. E:

- "não achei chamada direta" **não** prova que o método morreu:
  `package:js`, tearoffs e despacho dinâmico exigem tratamento conservador;
- alcançabilidade de **corpo executável**, de **dados** e de **informação
  de tipo** são separadas: uma classe pode precisar de identidade sem
  precisar de um construtor.

---

## 6. Otimização global sem carregar tudo (ThinLTO)

Um resumo por biblioteca — declarações e assinaturas, referências, classes
instanciadas, chamadas e efeitos, constantes relevantes, impressões
digitais de corpos candidatos — e um índice global que decide o que é
alcançável e quais corpos merecem ser carregados.

É arquitetura inspirada no ThinLTO, não uso de LLVM no caminho
Dart → JavaScript.

---

## 7. Ordem das otimizações

Depois de IR correta e alcançabilidade: SCCP, CSE/GVN, devirtualização,
inlining com orçamento, substituição escalar de agregados, e só então
fusão de funções. Fusão de funções **parecidas** (Sequence Alignment,
HyFM) fica para um modo orientado a tamanho, decidido por bytes emitidos,
bytes comprimidos e execução no navegador — não por instruções removidas
da IR.

Equality saturation (`egg`, POPL 2021) é interessante e fica para depois,
com limites de tempo e de nós, respeitando semântica numérica, exceções e
efeitos do Dart.

---

## 8. Emissão de JS para as engines

- **Shapes/hidden classes**: inicializar sempre as mesmas propriedades na
  mesma ordem, dentro do construtor. Atribuição condicional fora dele
  degrada o inline cache de monomórfico para megamórfico.
- **Despacho plano** para o que sobrar polimórfico, em vez de cadeias
  longas de protótipo.
- **Minificação por frequência**: os símbolos mais frequentes recebem os
  nomes mais curtos.

---

## 9. Como provar que melhorou

| cenário | o que medir |
|---|---|
| compilação inicial sem cache | tempo por fase, pico de RAM, alocações |
| compilação sem alterações | quanto trabalho ainda acontece à toa |
| edição pequena de corpo | consultas invalidadas, bibliotecas reemitidas |
| edição de assinatura/constante/anotação | correção da propagação |
| edição de HTML/CSS do ngdart | trabalho de codegen e recompilação |
| pacote copiado 1, 2, 4 e 8 vezes | crescimento de parse, memória e JS emitido |
| centenas de edições no daemon | retenção de revisões e estabilidade da memória |

Para deduplicação, o teste é de **comportamento**, não de texto: identidade
de tipos, estado independente, funções como valores, inicialização,
exceções e referências vindas de código gerado. Onde o Dart permite mais de
um resultado válido (identidade de closures, por exemplo), o teste aceita o
conjunto, não uma implementação específica.

---

## 10. Um relatório de duplicação, não só um número

Mais útil para depurar o otimizador do que "economizados 30 KB":

```text
Conteúdo compartilhado entre 3 arquivos.
Bibliotecas preservadas separadamente:
  - identidades de tipos distintas
  - armazenamento global distinto
Corpos compartilhados: 18 funções
Corpos não compartilhados:
  - 4 dependem de estado diferente
  - 2 não produziram economia
```
