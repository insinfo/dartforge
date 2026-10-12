# Comparativo nativo: DartForge × Dart, Go, TinyGo, Swift, Perry e scriptc

Medido em 2026-10-12, num Mac mini (macOS 27.2, arm64). O objetivo é usar
compiladores nativos maduros como referência para melhorar o DartForge,
principalmente em desempenho e tamanho do executável.

## Como foi medido

Os mesmos quatro núcleos foram escritos em Dart, Go, Swift e TypeScript
(`bench/comparativo/`), mais um "olá" em cada linguagem:

| núcleo | o que exercita |
|---|---|
| `fib(32)` | chamadas recursivas (custo de prólogo e epílogo) |
| `arvores(14)` | ~3,1 milhões de objetos pequenos alocados e descartados |
| `numerico` | laço de 30 milhões de iterações com `*`, `+` e `%` em inteiros |
| `textos` | 1 milhão de interpolações `"k" + i` e um mapa de 5 mil chaves |

Cada núcleo roda 4 vezes. Vale a mediana das rodadas depois da primeira (o
aquecimento). Os resultados dos núcleos são iguais em todas as ferramentas.

Para refazer a medida, com as ferramentas nas variáveis de ambiente descritas
no próprio script:

```
python3 scripts/comparar-linguagens.py target/comparativo
```

Ferramentas e opções:

| ferramenta | versão | opções |
|---|---|---|
| DartForge | `main` de 2026-10-12, modo padrão (tracing, A0) | `dartforge aot --optimize` |
| Dart | 3.6.2 | `dart compile exe` |
| Go | 1.27.2 | `go build -trimpath -ldflags="-s -w"` |
| TinyGo | 0.43.0 | `tinygo build -opt=2 -no-debug` |
| Swift | 6.4 (Xcode) | `swiftc -O` |
| Perry | 0.5.1520 | `perry compile`, com o runtime recompilado da fonte (`PERRY_WORKSPACE_ROOT`) |
| scriptc | 0.2.10 (Vercel Labs) | `scriptc build` |

**Ressalvas:**
- Uma rodada por ferramenta, numa máquina em que outros builds rodavam ao
  lado: conte com variação de ~10%.
- No TypeScript os números são `double`. Isso explica o `numerico` lento do
  Perry (resto em ponto flutuante); no scriptc o tipo inferido também pesa.
- No `numerico`, o otimizador do Swift resolveu o laço em tempo de compilação
  (0 ms); o número não mede o laço.

## Resultados

### Velocidade, memória e partida

| ferramenta | `fib` (ms) | `arvores` (ms) | `numerico` (ms) | `textos` (ms) | RSS do bench | 10× "olá" |
|---|---:|---:|---:|---:|---:|---:|
| **DartForge** | 18 | 38 | 106 | 83 | 19 MB | 0,26 s |
| Dart 3.6.2 | 9 | **11** | 108 | 49 | 53 MB | 0,32 s |
| Go | 4 | 24 | **86** | 39 | 12 MB | 0,22 s |
| TinyGo | **3** | 48 | 92 | 47 | 6 MB | 0,19 s |
| Swift | 5 | 78 | 0 (resolvido na compilação) | 37 | 8 MB | 0,19 s |
| Perry | 5 | 35 | 1.122 | 54 | 32 MB | 0,27 s |
| scriptc | 16 | 21 | 234 | **23** | **4 MB** | **0,19 s** |

### Tamanho do executável do "olá"

| ferramenta | tamanho | o que vai dentro |
|---|---:|---|
| Swift (macOS) | 51 KB | só o código; o runtime Swift é do sistema operacional |
| scriptc | 73 KB | runtime mínimo; só depende da `libSystem` |
| TinyGo | 131 KB | runtime e GC próprios, mínimos |
| Go | 1,6 MB | runtime completo (escalonador, GC, reflexão) |
| **DartForge** | **3,1 MB** | runtime Rust e SDK compilado |
| Dart 3.6.2 | 5,3 MB | VM de AOT e snapshot |
| Swift estático (Linux musl) | 5,8 MB | runtime Swift embutido, sem símbolos |
| Perry, runtime recompilado da fonte | 8,3 MB | runtime e stdlib Rust |
| Perry, binário oficial | 14,2 MB | stdlib inteira |

**Swift estático:**
- No macOS o Swift recusa o runtime estático (`-static-stdlib is no longer
  supported for Apple platforms`).
- O número vem da compilação cruzada para Linux com o toolchain oficial 6.4.0 e
  o Static Linux SDK. É só tamanho: o binário Linux não roda nesta máquina.
- Com o runtime embutido, o Swift fica maior que o DartForge.

**Perry:** anuncia ~330 KB para o "olá", mas isso não se reproduziu:
- 8,3 MB com o runtime recompilado da fonte, no modo de menor tamanho;
- 14,2 MB com o binário oficial.

## Onde o DartForge está

| aspecto | situação | referência |
|---|---|---|
| tamanho | 1,7× menor que o Dart; 2,6× menor que o Perry; 24× maior que o TinyGo | TinyGo, scriptc |
| chamadas (`fib`) | o mais lento (18 ms), 2× o Dart e 4–6× Go, TinyGo e Swift | Go, TinyGo |
| alocação (`arvores`) | 3,5× o Dart; 1,6× o Go | Dart, scriptc |
| strings e mapa (`textos`) | o mais lento: 1,7× o Dart, 3,6× o scriptc | scriptc, Swift, Go |
| aritmética (`numerico`) | empata com o Dart, 1,2× o Go | — |
| memória | 2,8× menos que o Dart; 1,6× mais que o Go | TinyGo, scriptc |
| partida | mais rápida que Dart e Perry; perto de Go e TinyGo | — |

## O que aprender de cada projeto (prioridades para o DartForge)

### 1. Custo de chamada (`fib`: 18 ms contra 3–5 ms)

Go e TinyGo pagam por chamada só a conferência de pilha no prólogo. O DartForge
paga hoje, em toda função:
- a leitura do contexto (`dartforge_contexto`);
- empilhar e desempilhar o quadro da pilha-sombra;
- conferir a pendência de exceção depois de cada chamada;
- o ponto seguro nos laços.

Caminhos já existentes no projeto:
- **Raízes por mapas de pilha** (`DARTFORGE_RAIZES=mapas`, modo B) no lugar da
  pilha-sombra, como o Go faz com os *stack maps*.
- **Exceções por tabelas** (`--excecoes=tabelas`, modo A1/B1) no lugar da
  conferência depois de cada chamada.
- A medida de 2026-10-10 já dava B1/A0 = 0,95.
- **Funções folha** que não alocam nem chamam o runtime não precisam de quadro
  nem de contexto: emitir o prólogo só quando necessário.

### 2. Alocação (`arvores`: 38 ms contra 11 ms do Dart)

O Dart vence com alocação por incremento de ponteiro em linha e coleta jovem
barata. O scriptc e o Go também ficam abaixo de 25 ms. A investigar:
- a fatia de tempo da coleta jovem (copiadora ou não, frequência, custo por
  objeto vivo);
- se toda alocação de objeto do programa sai pela TLAB em linha;
- o custo do `?.` com `??` em `int?`, que encaixota o inteiro no caminho
  nulo-ciente (no Swift e no Go não há caixa).

### 3. Strings e mapas (`textos`: 83 ms contra 23 ms do scriptc)

O núcleo é interpolação de `int` em string, hash e mapa de chave `String`.
Referências:
- O scriptc e o Swift guardam strings curtas sem alocação no heap.
- Go e Swift têm mapas especializados no tipo da chave.

Para o DartForge:
- converter `int` para texto e concatenar num só passo, sem objetos
  intermediários;
- guardar o hash na string;
- um caminho de mapa especializado para chave `String` sem passar por despacho
  dinâmico de `==`/`hashCode` quando a classe é a do runtime.

### 4. Tamanho (3,1 MB contra 131 KB do TinyGo e 73 KB do scriptc)

TinyGo e scriptc mostram que um runtime mínimo cabe em dezenas ou centenas de
KB. O que pesa no DartForge é o runtime Rust e o SDK compilado, ligados mesmo
quando o programa não os usa. Direções:
- podar o runtime por uso (mundo fechado também sobre o runtime);
- `panic=abort` e `opt-level="z"` nas partes frias;
- ICF;
- metadados (rastro, mapas, RTI) compactos ou opcionais.

O trabalho em andamento sobre o tamanho do nativo está na seção correspondente
do `ESTADO.md`. A meta do projeto é ficar abaixo do `dart compile exe`, o que
já vale no "olá" (3,1 contra 5,3 MB), e seguir em direção ao TinyGo.

### 5. Memória

Go e TinyGo usam 12 e 6 MB, contra 19 MB do DartForge. Vale medir o tamanho
inicial do heap e da TLAB, e o limiar da primeira coleta.

## Fontes sobre Perry e scriptc

- [PerryTS/perry](https://github.com/PerryTS/perry): Rust, SWC e LLVM. O README anuncia ~330 KB.
- [Perry: deep dive](https://fr0stb1rd.is-a.dev/posts/perry-typescript-native-compiler-deep-dive/)
- [scriptc](https://github.com/gching/scriptc): experimental, da Vercel Labs (o nome às vezes aparece como "Script C").
- [Vercel scriptc (RuntimeWire)](https://runtimewire.com/article/vercel-scriptc-typescript-native-compiler-no-javascript-engine)
- [I tried to compile TypeScript into a native binary with scriptc (DEV)](https://dev.to/remojansen/i-tried-to-compile-typescript-into-a-native-binary-with-scriptc-5d07)
