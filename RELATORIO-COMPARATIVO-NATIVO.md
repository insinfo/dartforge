# Comparativo nativo: DartForge × Dart, Go, TinyGo, Swift, Perry e scriptc

Medido em 2026-10-12, num Mac mini (macOS 27.2, arm64). O objetivo é usar
compiladores nativos maduros como referência para melhorar o DartForge,
principalmente em desempenho e tamanho do executável.

Há três partes:
1. núcleos de CPU e memória, comparados entre linguagens;
2. as variantes do próprio DartForge (modos de raízes e exceções, ARC,
   otimização, JIT);
3. um servidor HTTP com JSON.

## Como foi medido

### Núcleos

Os mesmos cinco núcleos foram escritos em Dart, Go, Swift e TypeScript
(`bench/comparison/bench.*`), mais um "olá" em cada linguagem:

| núcleo | o que exercita |
|---|---|
| `fib(32)` | chamadas recursivas (custo de prólogo e epílogo) |
| `arvores(14)` | ~3,1 milhões de objetos pequenos alocados e descartados |
| `numerico` | laço de 30 milhões de iterações com `*`, `+` e `%` em inteiros |
| `textos` | 1 milhão de interpolações `"k" + i` e um mapa de 5 mil chaves |
| `json` | 50 mil objetos (inteiros, string, lista, booleano) codificados e decodificados |

- Cada núcleo roda 4 vezes. Vale o **menor tempo** das rodadas depois da
  primeira (o aquecimento), que é a medida menos sensível a outros processos.
- Os resultados de cada núcleo são iguais em todas as ferramentas que rodaram.
- O resultado do `json` soma campos dos objetos decodificados. Por isso não
  depende da formatação nem da ordem das chaves.

```
python3 scripts/compare-languages.py target/comparativo [ferramenta...]
```

| ferramenta | versão | opções |
|---|---|---|
| DartForge | `main` de 2026-10-12 | `dartforge aot --optimize` (variantes na seção 2) |
| Dart | 3.6.2 | `dart compile exe` |
| Go | 1.27.2 | `go build -trimpath -ldflags="-s -w"` |
| TinyGo | 0.43.0 | `tinygo build -opt=2 -no-debug` |
| Swift | 6.4 (Xcode) | `swiftc -O` |
| Perry | 0.5.1520 | `perry compile`, com o runtime recompilado da fonte (`PERRY_WORKSPACE_ROOT`) |
| scriptc | 0.2.10 (Vercel Labs) | `scriptc build` |

### Servidor HTTP

O mesmo servidor em cada linguagem (`bench/comparison/server.*`, o
protocolo de `bench/http/server.dart`):
- `GET /` responde `hello`;
- `GET /json` responde um objeto pequeno codificado na hora.

| linguagem | servidor |
|---|---|
| Dart | `dart:io` |
| Go | `net/http` |
| Swift | SwiftNIO (`swift-servidor/`) |
| TypeScript | `node:http` (Perry e scriptc) |

- A carga vem de `bench/comparison/load.go`: clientes com keep-alive em laço
  por 5 s, com 1 e 32 conexões. Ele substitui o `wrk`, que não está instalado.
- Além de req/s e latência, mede-se a CPU do servidor por mil requisições.
  É a medida mais justa entre servidores de uma thread (Dart) e de várias
  (Go, Swift).

```
python3 scripts/compare-http.py target/comparativo-http [servidor...]
```

### Ressalvas

- Uma ou duas rodadas por ferramenta, numa máquina em que outros builds
  rodavam ao lado: conte com variação de 10–30%. Entre duas rodadas, o A0 do
  DartForge variou de 83 a 144 ms no `textos`. As tabelas usam a rodada mais
  limpa de cada linha.
- **No TypeScript os números são `double`.** Isso explica o `numerico` lento do
  Perry (resto em ponto flutuante).
- **No `numerico`, o otimizador do Swift resolveu o laço em tempo de
  compilação** (0 ms); o número não mede o laço.
- **O servidor Dart roda num isolado só (uma thread)**; Go, SwiftNIO e o
  scriptc usam vários núcleos. Com 32 conexões isso pesa mais que o compilador.
  A coluna de CPU por requisição compensa essa diferença.
- O gerador de carga roda na mesma máquina e disputa os núcleos.

## 1. Núcleos entre linguagens

| ferramenta | `fib` | `arvores` | `numerico` | `textos` | `json` | RSS do bench | 10× "olá" |
|---|---:|---:|---:|---:|---:|---:|---:|
| **DartForge (A0)** | 16 | 34 | 96 | 83 | 112 | 75 MB | 0,13 s |
| Dart 3.6.2 | 9 | **11** | 95 | 45 | 70 | 188 MB | 0,21 s |
| Go | 6 | 21 | **76** | 35 | 91 | 72 MB | 0,24 s |
| TinyGo | **3** | 52 | 102 | 64 | 214 | 131 MB | — |
| Swift | 4 | 106 | 0 (na compilação) | 46 | 231 | 128 MB | 0,24 s |
| Perry | 8 | 31 | 1.008 | 51 | 57 | 197 MB | 0,16 s |
| scriptc | 15 | 21 | 224 | **21** | **13** | **36 MB** | 0,18 s |

Tempos em ms.
- TinyGo e Swift vêm da rodada completa; as outras linhas, da rodada mais limpa.
- O RSS inclui os 50 mil objetos do `json`, por isso subiu em relação à primeira
  versão deste relatório.

### Tamanho do executável do "olá"

| ferramenta | tamanho | o que vai dentro |
|---|---:|---|
| Swift (macOS) | 51 KB | só o código; o runtime Swift é do sistema operacional |
| scriptc | 73 KB | runtime mínimo; só depende da `libSystem` |
| **DartForge sem `--optimize`** | **123 KB** | o SDK fica numa biblioteca dinâmica à parte (perfil de desenvolvimento) |
| TinyGo | 131 KB | runtime e GC próprios, mínimos |
| Go | 1,6 MB | runtime completo (escalonador, GC, reflexão) |
| **DartForge (`aot --optimize`)** | **3,1 MB** | runtime Rust e SDK compilado, num arquivo só |
| Dart 3.6.2 | 5,3 MB | VM de AOT e snapshot |
| Swift estático (Linux musl) | 5,8 MB | runtime Swift embutido, sem símbolos |
| Perry, runtime recompilado da fonte | 8,3 MB | runtime e stdlib Rust |
| Perry, binário oficial | 14,2 MB | stdlib inteira |

**Swift estático:**
- No macOS o Swift recusa o runtime estático (`-static-stdlib is no longer
  supported for Apple platforms`).
- O número vem da compilação cruzada para Linux com o toolchain oficial 6.4.0 e
  o Static Linux SDK. É só tamanho: o binário Linux não roda nesta máquina.

**Perry:** anuncia ~330 KB para o "olá", mas isso não se reproduziu:
- 8,3 MB com o runtime recompilado da fonte, no modo de menor tamanho;
- 14,2 MB com o binário oficial.

## 2. As variantes do DartForge

| variante | o que é | "olá" | `fib` | `arvores` | `numerico` | `textos` | `json` | RSS | 10× "olá" |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| A0 | tracing, pilha-sombra, exceções por checagem (padrão) | 3,1 MB | 16 | 34 | 96 | 83 | 112 | 75 MB | 0,13 s |
| A0 sem `--optimize` | perfil de desenvolvimento (SDK em biblioteca dinâmica) | 123 KB | 20 | 85 | 288 | 160 | 359 | 89 MB | 0,58 s |
| A1 | A0 com exceções por tabelas | 3,2 MB | 16 | 45 | 130 | 118 | 160 | 77 MB | 0,27 s |
| B0 / B1 | raízes por mapas de pilha (sem/com tabelas) | 6,3 MB | 17 / 14 | **falha** | — | — | — | — | 0,30 s |
| ARC | `--memory arc` legado | 3,1 MB | 15 | 267 | 107 | 212 | 400 | 349 MB | 0,26 s |
| ARCP | ARC com a preparação publicada | **16,8 MB** | 14 | 427 | 105 | 396 | 954 | 350 MB | 0,43 s |
| ARCP + tabelas | ARCP com exceções por tabelas | 18,9 MB | 14 | 450 | 107 | 401 | 958 | 296 MB | 0,43 s |
| JIT | `dartforge run` (a partida inclui compilar) | — | 18 | 68 | 237 | 106 | 230 | 188 MB | 1,46 s |

- A0 vem da rodada mais limpa. As outras linhas vêm da rodada completa, mais
  ruidosa (nela o A0 deu 145 no `numerico` e 144 no `textos`).
- O JIT pede a CLI montada com `--features native,jit` (LLVM embutido). Para
  montá-la no macOS:
  - ligar com o `clang` e o `lld` do LLVM 22 (`CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER`
    e `-fuse-ld=lld`, como no CI);
  - passar uma `libzstd.a` (`-C link-arg=…/libzstd.a`). A distribuição do LLVM
    aponta para `/opt/homebrew/lib/libzstd.a`, que não existe nesta máquina.

**O que a tabela mostra:**
- **O ARC ainda é a variante mais lenta onde há alocação** (`arvores`, `textos`,
  `json`): 5–9× o A0. Esse é o trabalho em curso no `ESTADO.md`: RC no cabeçalho
  e retain/release em linha.
- **O ARCP gera executáveis de 16,8 MB**, contra 3,1 MB do A0. O `__text` tem
  15 MB contra 4 MB. A preparação conjunta do SDK liga o SDK inteiro, sem a poda
  de mundo fechado que o A0 faz. É um defeito a corrigir.
- **B0 e B1 caem com SIGSEGV** nas árvores, e o servidor HTTP em B1 morre com
  `handle além da tabela (escalar usado como handle)`. Ver a seção 4.
- **A1 é tão bom quanto A0** aqui, e melhor no servidor HTTP (seção 3).

## 3. Servidor HTTP e JSON

Requisições por segundo com 1 e 32 conexões, CPU do servidor por mil
requisições em `/json` com 32 conexões, tamanho do executável e RSS sob carga:

| servidor | `/` 1 con. | `/` 32 con. | `/json` 32 con. | CPU ms / mil req | executável | RSS sob carga |
|---|---:|---:|---:|---:|---:|---:|
| **DartForge A0** | 9.299 | 26.919 | 26.300 | 45 | 4,4 MB | 18 MB |
| **DartForge A1** | 18.965 | 33.924 | 30.084 | 41 | 4,5 MB | 18 MB |
| DartForge B1 | — | — | — | — | 8,8 MB | **cai** |
| DartForge ARC | 9.538 | 6.934 | 5.430 | 189 | 4,3 MB | 77 MB |
| DartForge ARCP | 3.494 | 4.911 | 4.129 | 250 | 16,0 MB | 82 MB |
| DartForge JIT | 9.600 | 18.826 | 18.429 | 62 | — | 94 MB |
| Dart AOT | 11.829 | 33.712 | 29.876 | 39 | 5,4 MB | 17 MB |
| Dart VM (JIT) | 18.354 | 34.383 | 28.943 | 41 | — | 179 MB |
| Go | 13.513 | 143.751 | 150.018 | 22 | 5,9 MB | 22 MB |
| Swift (SwiftNIO) | 39.811 | **191.067** | **176.887** | 27 | 7,0 MB | 9 MB |
| Perry | 11.660 | 75.329 | 86.754 | 10 | 15,1 MB | 32 MB |
| scriptc | **41.354** | 172.417 | 173.555 | **5** | **0,3 MB** | **2 MB** |
| TinyGo | não roda | | | | | |

O TinyGo compila o servidor, mas no macOS não tem pilha de rede
(`panic: … Netdev not set`).

**O que a tabela mostra:**
- **DartForge A1 empata com o Dart oficial** (AOT e VM) e usa a mesma memória do
  Dart AOT, num executável menor.
  - A0 é 25–100% mais lento que A1 no servidor: as exceções por tabelas tiram a
    conferência de pendência depois de cada chamada, e o código de `dart:io` é
    cheio de chamadas.
- **Os dois Darts param em ~30 mil req/s com 32 conexões**, porque o isolado é
  uma thread só. Go, SwiftNIO e scriptc passam de 140 mil usando todos os
  núcleos.
  - Mesmo por requisição, gastam 2–9× menos CPU: 5–27 ms por mil requisições,
    contra 39–45 dos Darts.
- **O scriptc é o mais eficiente**: 5 ms de CPU por mil requisições, 2 MB de RSS
  e um executável de 0,3 MB.
- **O ARC é 4–7× mais lento que o tracing no servidor.** O ARCP usa 5–6× mais CPU
  por requisição e 4× mais memória.

## 4. Defeitos do DartForge encontrados pela comparação

1. **Raízes por mapas de pilha (B0/B1) no macOS arm64:**
   - SIGSEGV no núcleo `arvores`;
   - o servidor HTTP morre na ~130ª requisição com keep-alive
     (`heap.rs:2452`: "handle além da tabela (escalar usado como handle)").
   - Um escalar está sendo tratado como raiz nos mapas. O modo foi validado no
     Windows; no macOS é a primeira medida com carga.
2. **ARC preparado sem poda do SDK:** executável de 16,8 MB contra 3,1 MB do A0
   (`__text` de 15 MB contra 4 MB).
3. **ARC lento em alocação e no servidor:** 5–9× o A0 nos núcleos que alocam,
   4–7× no servidor. O trabalho em curso (RC no cabeçalho, retain/release em
   linha) ataca isso.
4. **O JIT no macOS precisa de ajustes manuais para montar** (ligador do LLVM 22
   e `libzstd.a`). Vale tornar isso automático no `crates/llvm/build.rs`.

## 5. Onde o DartForge está

| aspecto | situação | referência |
|---|---|---|
| tamanho | 1,7× menor que o Dart; 2,7× menor que o Perry; 24× maior que o TinyGo; 43× maior que o scriptc | TinyGo, scriptc |
| chamadas (`fib`) | 16 ms: 2× o Dart, 3–5× Go, TinyGo e Swift | Go, TinyGo |
| alocação (`arvores`) | 3× o Dart; 1,6× o Go | Dart, scriptc |
| strings e mapa (`textos`) | 1,8× o Dart, 4× o scriptc | scriptc, Go |
| JSON | 1,6× o Dart; 1,2× o Go; 9× o scriptc | scriptc, Perry, Dart |
| aritmética (`numerico`) | empata com o Dart; 1,3× o Go | — |
| servidor HTTP | A1 empata com o Dart; 5–6× abaixo de Go e Swift com 32 conexões | Go, SwiftNIO, scriptc |
| memória | 2,5× menos que o Dart no bench; igual no servidor | scriptc, TinyGo |
| partida | mais rápida que Dart e Perry | — |

## 6. O que aprender de cada projeto (prioridades para o DartForge)

### 6.1 Custo de chamada (`fib` 16 ms contra 3–6 ms; servidor A0 contra A1)

Go e TinyGo pagam por chamada só a conferência de pilha no prólogo. O DartForge
paga hoje, em toda função:
- a leitura do contexto;
- empilhar e desempilhar o quadro da pilha-sombra;
- conferir a pendência de exceção depois de cada chamada;
- o ponto seguro nos laços.

A medida do servidor mostra o caminho:
- **exceções por tabelas (A1) já dão até 2× no servidor**, sem perder nos
  núcleos;
- tornar o A1 padrão é a mudança de maior retorno imediato;
- depois, as **raízes por mapas de pilha** (corrigindo o defeito da seção 4) e
  **funções folha sem quadro**.

### 6.2 Alocação (`arvores` 34 ms contra 11 ms do Dart)

O Dart vence com alocação por incremento de ponteiro em linha e coleta jovem
barata. O scriptc e o Go ficam abaixo de 25 ms. A investigar:
- a fatia de tempo da coleta jovem (copiadora ou não, frequência, custo por
  objeto vivo);
- se toda alocação de objeto do programa sai pela TLAB em linha;
- a caixa do `int?` no `?.` com `??` (no Swift e no Go não há caixa).

### 6.3 Strings, mapas e JSON (`textos` 83 contra 21 ms; `json` 112 contra 13 ms do scriptc)

- **Strings curtas:** o scriptc e o Swift as guardam sem alocação no heap.
- **Mapas:** Go e Swift especializam o mapa pelo tipo da chave.
- **JSON:** o scriptc gera o codificador e o decodificador a partir do tipo
  (`Item[]`), sem passar por mapas dinâmicos. O Perry também fica abaixo do Dart.

Para o DartForge:
- converter `int` para texto e concatenar num só passo;
- guardar o hash na string;
- mapa especializado para chave `String`;
- `jsonEncode`/`jsonDecode` em Rust no runtime para `Map`/`List` de primitivos.

### 6.4 Servidor HTTP

- **Já no nível do Dart oficial** (A1), com executável e memória menores.
- **Por requisição**, o caminho de `dart:io` gasta 41–45 ms de CPU por mil
  requisições, contra 5 do scriptc, 22 do Go e 27 do SwiftNIO. Medir onde vai
  esse tempo:
  - análise do cabeçalho;
  - cópias de buffer;
  - `Stream` e `Future` por requisição.
- **Uso de vários núcleos:** em Dart isso é trabalho do programa (isolados com
  `shared: true` no `bind`). Vale um exemplo e uma medida com N isolados.

### 6.5 Tamanho (3,1 MB contra 131 KB do TinyGo e 73 KB do scriptc)

TinyGo e scriptc mostram que um runtime mínimo cabe em dezenas ou centenas de
KB. Nos 3,1 MB do DartForge, quase tudo é runtime Rust e SDK compilado, ligados
mesmo sem uso; sem `--optimize` o executável do programa tem 123 KB. Direções:
- podar o runtime por uso (mundo fechado também sobre o runtime);
- `panic=abort` e `opt-level="z"` nas partes frias;
- ICF;
- metadados (rastro, mapas, RTI) compactos ou opcionais;
- a poda do SDK no ARC preparado (seção 4).

A meta do projeto é ficar abaixo do `dart compile exe`, o que já vale no "olá"
(3,1 contra 5,3 MB) e no servidor (4,4 contra 5,4 MB), e seguir em direção ao
TinyGo.

## 7. Fontes sobre Perry e scriptc

- [PerryTS/perry](https://github.com/PerryTS/perry): Rust, SWC e LLVM. O README anuncia ~330 KB.
- [Perry: deep dive](https://fr0stb1rd.is-a.dev/posts/perry-typescript-native-compiler-deep-dive/)
- [scriptc](https://github.com/gching/scriptc): experimental, da Vercel Labs (o nome às vezes aparece como "Script C").
- [Vercel scriptc (RuntimeWire)](https://runtimewire.com/article/vercel-scriptc-typescript-native-compiler-no-javascript-engine)
- [I tried to compile TypeScript into a native binary with scriptc (DEV)](https://dev.to/remojansen/i-tried-to-compile-typescript-into-a-native-binary-with-scriptc-5d07)

## 8. Continuação da implementação — 2026-10-13

Os resultados das tabelas acima permanecem históricos. A continuação
HTTP/JSON está em `RELATORIO-HTTP-JSON-AOT.md`: correção do unwind de JSON no
macOS arm64 com tabelas, remoção de buffers/cópias intermediários no Socket,
encoder UTF-8 reutilizado e medições com uma conexão local. Os ganhos dessa
carga não substituem o comparativo com 32 conexões nem podem ser somados.

A base do dart_ui também avançou com três subagentes: detecção AOT
`dartforge.compiler=true`, pragmas `vm:prefer-inline`/`vm:never-inline` e
`--subsystem=gui` durante a ligação Windows, com CRT GUI. São mudanças
implementadas e verificadas; ganho de CPU/tamanho por inline ainda não foi
medido. Validação integrada: 487 testes do emissor, três de CLI, ligação
cruzada PE GUI/console e 30 execuções AOT em tracing/ARC/ARC preparado,
com/sem GC stress, idênticas ao Dart para as sondas de correção.

A composição de pixels e a transferência de frames do dart_ui rodaram; esta
última confirmou visibilidade em 200 rodadas. O probe portátil de callback
mostrou zero ticks dentro da chamada nativa e diferença no catch-up após o
retorno (34 no DartForge, quatro na VM), a investigar. Não há medida nova de
rasterização, capacidade máxima HTTP, p95/p99 ou uso de múltiplos isolados.
Prioridades de alocação/mapas/JSON da seção 6 continuam abertas; a próxima
etapa de UI é wake+CFRunLoop, com AppKit e reentrância tratados separadamente.
Ver `RELATORIO-DARTFORGE-PARA-DART-UI.md` §9 e evidências em
`bench/results/2026-10-13-dart-ui-phase1/validation.json`.

## 9. Kernels reais e contratos de temporização — 2026-10-13

A prioridade de compatibilidade agora tem duas sondas que importam os kernels
atuais do dart_ui. Conversão de vídeo e rasterização de strips produziram
9.067/33.201 bytes iguais na VM, no AOT oficial e em 12 execuções nativas
(tracing/ARC/ARC preparado, normal/GC stress). O runner reproduzível é
`scripts/verify-dart-ui.py`; a saída padrão verifica correção. Benchmark de
strips é separado, inclui consumo de todos os registros/alphas e teve apenas
smoke funcional. Não há novos tempos para substituir as tabelas históricas.

Também corrigida a entrega em rajada dos timers após bloqueio: períodos
atrasados são pulados antes do callback e contados em `Timer.tick`, segundo
o SDK 3.6.2. As regressões `142`/`143` passaram em 12 execuções AOT com os
mesmos dois oráculos. O probe real passou de 34 para quatro callbacks após
o `qsort` nesta rodada; dentro da chamada, continuaram zero. Gates das partes
alteradas: 494 testes do emissor e 349 do runtime, incluindo ignorados,
sem falhas. Ver `RELATORIO-DARTFORGE-PARA-DART-UI.md` §10 e
`bench/results/2026-10-13-dart-ui-corpus/validation.json`.

## 10. Consolidação do pacote e corpus FFI — 2026-10-13

SIMD foi movido sem alteração para `packages/dartforge`; o pacote anterior
foi removido por solicitação do proprietário. O probe de overflow, shifts,
comparações e máscaras manteve 812 bytes idênticos à VM/AOT oficial, normal
e GC stress no nativo, e operações `<4 x i32>` no IR. Não foi repetida a
medição histórica de blend: preservação de resultado/IR não mede ganho.

O corpus real do dart_ui agora inclui FFI, além de vídeo e strips:
18 execuções nativas nos seis modos, idênticas às duas referências.
As anotações const passaram nos seis modos, inclusive ARC preparado após
explicitar o ownership escalar de `Timer.tick`. A bomba CFRunLoop/fila
principal passou em 12 execuções de wake e ciclo de vida. Não há medida
comparativa de CPU ociosa, latência AppKit ou entrada de áudio certificada.

Gates das partes alteradas: 496 testes do emissor e 353 do runtime passaram,
incluindo ignorados por padrão. `cargo fmt --check` continua sem aprovação
global. Evidências, hashes e flags em
`bench/results/2026-10-13-dartforge-package/`; detalhes da entrega em
`RELATORIO-DARTFORGE-PARA-DART-UI.md` §11 e `ESTADO.md`.

## 11. Progresso AppKit e política de contratos — 2026-10-13

O dispatcher nativo permite chamar o backend AppKit entre turnos Dart.
Seis execuções tracing/ARC/ARC preparado, normal/stress, confirmaram
NSApplication/delegate, evento sintético, timer/mensagem, falha de lançamento
com cleanup e encerramento. Não houve janela/rendering nem medição de frames.

A0 reconhece metadata, mas não prova propriedades: verify recusa obrigações
pendentes e ignore registra ausência de certificação. Relatórios têm origem/
hashes/política/artefato, com nomes distintos inclusive para arquivos que
diferem apenas por extensão. Mudanças de metadata/política entram no cache
sem vincular a emissão ao diretório físico do checkout.

Os novos testes não substituem benchmarks HTTP/JSON/vídeo nem certificam
entrada de áudio. Detalhes e limites em `ESTADO.md` e relatório dart_ui §12;
dados em `bench/results/2026-10-13-dartforge-package/`.

## 12. Recursos portáveis e memória mapeada — 2026-10-13

Bibliotecas por candidatos, liberação síncrona/aguardada e mapeamento de
arquivos passaram em 18 execuções nativas iguais à VM/AOT oficial no macOS.
O runner registra fontes, artefatos, modos e estabilidade de entradas em
`resources-and-mapping-validation.json`; a análise do pacote passou.
`MappedBytes` confere lifetime por acesso; o empréstimo de Uint8List externa
sem cópia depende de contrato síncrono sem escape. Windows/Linux aguardam
execução própria. Esses testes não medem alocações, throughput ou latência
de HTTP/JSON e não substituem a comparação pareada já documentada.

## 13. Predicados nullable e ciclo de vida FFI — 2026-10-13

O otimizador normaliza somente literais exatos de null/boolean em comparações
com representações Ref/I1 conhecidas. Propagação de null e identidades booleanas
eliminam dispatch morto em dois predicados reais com parâmetro nullable,
preservando os produtores com efeito dos operandos eager. Seis execuções
nativas tracing/ARC/preparado, normal/stress, produziram os mesmos 87 bytes
que VM/AOT oficial; três IRs mantêm comparação real do parâmetro com null e
nenhum dispatch de igualdade nesses predicados. Isso é evidência funcional
e estrutural, **não ganho medido de HTTP/JSON ou redução geral de alocações**.

P5 define chamadas após close em código AOT residente com tombstones nativos;
o custo é memória de contexto/trampolim conservada até o fim do processo.
O recorte não habilita callback local vivo em thread estrangeira, realtime
ou unloading JIT. Checkers SSA internos também não certificam NoEscape AOT.
Ver `ESTADO.md` e os registros de validação do pacote; medições pareadas de
throughput/alocações permanecem necessárias antes de atribuir ganho ao servidor.
