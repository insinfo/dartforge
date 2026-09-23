# Experimento futuro — ARC com coleta de ciclos no backend nativo

**Estado: registrado, não iniciado.** Fora da trilha atual; nada aqui é
compromisso nem prazo. Entra na fila só quando as pré-condições do §7 forem
verdadeiras. Registrado em 2026-09-23.

## 1. A pergunta

Dá para o backend nativo (trilha nova → HIR → LLVM IR → Clang, runtime Rust)
gerenciar a memória dos objetos Dart por **contagem de referências (ARC)**
em vez do GC por tracing de hoje — **sem mudar a sintaxe nem a semântica do
Dart**?

Resposta de trabalho: **sim para a sintaxe; a semântica exige ARC com
coleta de ciclos, não ARC puro.** As operações de `retain`/`release` são
introduzidas pelo compilador e pelo runtime; o programador continua
escrevendo Dart comum. Isso é coerente com duas regras do `PLANO.md`:
"Nada de sintaxe nova" e "equivalência semântica com o Dart oficial".

## 2. Por que ARC puro não serve

Dart permite ciclos de referências fortes:

```dart
class No { No? proximo; }
void criarCiclo() {
  final a = No(); final b = No();
  a.proximo = b; b.proximo = a;   // ao sair, nenhum contador chega a zero
}
```

ARC puro vaza esse par. O Objective-C ARC declara explicitamente que não
coleta ciclos e deixa isso ao programador (referências `weak`); aqui isso
obrigaria a reescrever aplicações e pacotes — viola a regra de
compatibilidade com o ecossistema. **Enfraquecer referências
automaticamente** também não serve sem prova de que preserva o
comportamento: liberaria objetos ainda alcançáveis.

Ciclos não vêm só do código do usuário: ambientes de closure, estados de
funções `async` e registros de callback de `Future` formam ciclos no
próprio runtime (o Nim documenta que o seu `async` padrão vaza sem coletor
de ciclos).

## 3. Desenho a experimentar

```
Dart comum
  → HIR com ownership (owned / guaranteed, transferência, fim de vida)
  → otimização de retain/release (eliminar redundantes, reconhecer transferências)
  → LLVM IR
  → código nativo + runtime: contadores, descritores de campo, weak,
    e coletor de ciclos (trial deletion, como o ORC do Nim)
```

* **Ownership fica na IR, não na linguagem.** Modelo de referência: Ownership
  SSA do Swift (`references/swift/docs/OwnershipManifesto.md`, e o verificador
  de ownership no SIL) — categorias owned/guaranteed permitem não tocar o
  contador em acessos temporários e ter um **verificador** que rejeita uso
  depois de liberar e caminhos desequilibrados (mesmo espírito do verificador
  de HIR do contrato atual).
* **Coleta de ciclos no runtime**, complementar à contagem: identifica grupos
  inalcançáveis que se mantêm vivos entre si. Continua sendo gerenciamento
  automático de memória com coleta de lixo — não vender como "sem GC".
* **Contadores não atômicos por isolate**: o modelo de isolates com heap
  isolado permite contagem sem atomics dentro do isolate. FFI, transferência
  entre isolates e memória compartilhada precisam de análise própria.
* **Substituição de campo**: reter o novo valor antes de liberar o antigo
  (os dois podem ser o mesmo objeto).
* **Convenção entre funções**: quem mantém vivos argumentos e resultados, em
  chamada direta, dinâmica, retorno antecipado, exceção pendente e chamada
  nativa.

## 4. O que tem de continuar exatamente como no Dart

| contrato | por que ARC ingênuo quebra | o que o runtime precisa |
| --- | --- | --- |
| identidade (`identical`, `b = a` compartilha, `const` canonicalizado) | tentação de copiar por valor | ARC muda só quando liberar, nunca a semântica de referência |
| closures, `Future.then`, estado após `await` | callbacks e estados retidos pelo runtime | ambientes e estados `async` como objetos gerenciados, participando da coleta de ciclos |
| `WeakReference` | referência fraca não pode manter vivo | registro de weak refs zerado na liberação |
| `Expando` | mapa forte retém o valor para sempre | tabela efêmera (ephemeron) |
| `Finalizer` | contador a zero ≠ rodar callback Dart ali | callback agendado **como evento** (não síncrono, não microtask); a API nem garante execução |
| `Finalizable` (`dart:ffi`) | "último uso ⇒ libera" viola a garantia | local com tipo `Finalizable` vive até o fim do bloco; regras para `this`, closures e `async` |
| `NativeFinalizer` | não é o mesmo contrato do `Finalizer` | garantias no encerramento do grupo de isolates |
| `dispose()`/`close()` | não são destrutores | nunca chamar implicitamente |

## 5. Expectativas honestas

* **Não garante** mais velocidade, menos memória nem ausência de pausas:
  liberar a raiz de uma árvore grande libera a árvore inteira de uma vez; o
  custo muda de lugar. Atualizar contadores tem custo — o ganho depende de
  eliminar retain/release redundantes (referência: Perceus/Koka, com a
  ressalva de que as garantias dele partem de um núcleo funcional e não se
  transferem a objetos Dart mutáveis).
* **Tempo real (áudio)**: ARC sozinho não resolve — `release` final, `free` e
  o alocador têm tempo imprevisível (o RealtimeSanitizer do Clang trata
  `malloc`/`free`/mutex como proibidos em região de tempo real). Isso pede um
  caminho separado, com buffers pré-alocados, não uma troca de GC.
* **Hot reload** (`docs/PESQUISA-HOT-RELOAD.md`): ARC não impede nem
  implementa. Mudança de layout de classe exige descritores de campo e rotinas
  de liberação **por versão de layout** — nunca interpretar memória antiga com
  o formato novo.
* **O LLVM não faz isso por nós**: o ARC do Clang é para objetos Objective-C e
  blocos, com o runtime deles. E o borrow checker do Rust protege o
  compilador, não a vida dos objetos Dart no executável.

## 6. Relação com o que está sendo feito agora

O contrato de representação e raízes do nativo (`docs/NATIVO-PLANO.md`,
seção "Contrato de representação e raízes": R/E/N/G) é **pré-requisito**, não
concorrente:

* **R** (representação: `Ref` é só null ou handle vivo; escalares encaixotados
  com `Box`/`Unbox`) e **E** (toda aresta do heap marcada como referência) são
  exatamente os descritores de campo que ARC e o coletor de ciclos precisam.
* **G** (raízes explícitas com slots por SSA) é o que o ARC **substituiria**
  pela convenção owned/guaranteed; o verificador de HIR é o mesmo lugar onde
  entraria o verificador de ownership.
* Com um contrato só, os dois gerenciadores podem coexistir atrás de uma
  opção (`--memoria tracing|arc`) e ser comparados no mesmo corpus.

## 7. Pré-condições para iniciar

1. Contrato R/E/N/G implementado e o corpus nativo em patamar alto (hoje
   7/214) — medir ARC contra um tracing que não funciona não mede nada.
2. `async`, closures e `Future` no backend nativo (é onde os ciclos do runtime
   aparecem).
3. `WeakReference`, `Expando` e `Finalizer` existindo no tracing, com testes —
   viram o oráculo do comportamento que o ARC tem de preservar.
4. Referências baixadas e estudadas antes de desenhar: Nim (ARC/ORC, trial
   deletion), Koka (Perceus), Swift (Ownership SSA, SIL ownership verifier,
   runtime de retain/release em `stdlib/public/runtime`), e o coletor de
   ciclos de Bacon & Rajan. (Nim e Koka ainda não estão em `references/`.)

## 8. Como seria provado

* O **mesmo corpus diferencial** nativo, com `--memoria arc`: saída byte a
  byte igual à VM, inclusive com `--gc-stress` equivalente (forçar coleta de
  ciclos a cada alocação).
* Casos novos de corpus: ciclos diretos e via closure/`async`, `WeakReference`
  e `Expando` com alvo liberado, `Finalizer` rodando como evento,
  `Finalizable` vivo até o fim do bloco.
* Medição contra o tracing no mesmo programa: tempo, pico de memória e
  distribuição de pausas — decidir por número, não por expectativa.
