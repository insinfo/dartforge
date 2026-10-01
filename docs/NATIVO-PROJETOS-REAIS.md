# Backend nativo em projetos reais: o new_sali/backend e o pub

Levantamento, especificação e estado das correções para o backend nativo
compilar e rodar um backend Dart real grande (o `new_sali/backend`: angel3,
`dart:io`, isolados, FFI, banco de dados) e os exemplos e testes de pacotes
populares do pub, comparados com a VM. Escrito em 2026-09-30.
`EN/` é `crates/emit_native/src/`, `RT/` é `crates/runtime/src/`, `VM/` é
`E:\references\dart-sdk\`.

## 0. Como foi medido

**O backend.** `C:\MyDartProjects\new_sali\backend` é só leitura: a cópia de
trabalho fica em `E:\dftemp\backend-real\backend` (com o `core/` ao lado).
As dependências foram resolvidas com um `PUB_CACHE` próprio
(`E:\dftemp\backend-real\pubcache`, `dart pub get`), porque o pub-cache da
máquina não tem os `angel3_*`; a dependência git `dart_pdf` aponta para o
clone local `C:\MyDartProjects\insinfo_dart_pdf` (o `ref` do `pubspec`
não existe mais no GitHub), e as dependências de desenvolvimento que não
resolvem sem rede (`pdfium_bindings`, `pdfbox_dart`, `mocktail`…) saíram
da cópia do `pubspec.yaml`. O `.env` aponta para a cópia. Entrada:
`bin/prod.dart` (o `dart compile exe` de produção do projeto).

O banco: o Postgres 17 e o Redis da máquina são serviços **desabilitados**
(`Get-Service`: `Stopped`, `Disabled`); não foram ligados. Na VM o backend
sobe (`Servidor escutando em http://127.0.0.1:3390`), o `/metrics`
(Prometheus) responde 200, e as rotas que passam pelo banco ou pelo Redis
não respondem (o isolado de e-mail morre com `SocketException` ao
conectar no Redis, e o pool tenta reconectar).

**O pub.** `E:\dftemp\backend-real\pub` é um projeto que depende de 78
pacotes do pub-cache da máquina (resolução `--offline`); os pacotes são
copiados para `E:\dftemp\backend-real\pkgs` e cada programa roda com o
diretório do pacote como corrente. Programas: todo `example/*.dart` com
`main` e os 3 menores `test/**/*_test.dart` de cada pacote — 272. O
roteiro `E:\dftemp\backend-real\placar.py` roda cada um na VM
(`dart --enable-asserts --packages=… arquivo`, o mesmo oráculo do
corpus, `crates/diferencial/src/oraculos.rs:260`) e no nativo
(`dartforge aot`), e compara a saída padrão (sem os tempos `00:00` do
`package:test` e as sementes `Random Seed: N` que os testes imprimem) e o
código de saída. Programa que a VM não compila (código 254: dependência
de teste ausente, como `test_descriptor`) fica fora do placar. O motor de
build do dartforge é desligado no roteiro
(`DARTFORGE_BUILD_COMPILANDO_EXECUTOR=1`): os pacotes publicados já trazem
o código gerado, e o motor tomaria a cópia do pacote como raiz e cobraria
as dependências de desenvolvimento dela.

**O perfil.** Sem `perf` no Windows: `E:\dftemp\backend-real\amostrar.py`
suspende cada thread do processo, percorre a pilha com o `StackWalk64` da
`dbghelp` e simboliza pelo PDB (um build com
`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`, sem LTO).

## 1. Causas (por número de programas destravados)

| | causa | onde apareceu | programas |
|---|---|---|---|
| C1 | emissão quadrática: busca linear de função por símbolo a cada `CallStatic`, de implementações varrendo todas as classes a cada acesso a membro, e o ponto fixo de exceções por rodadas | new_sali/backend (46 min sem terminar) | o backend |
| C2 | membro estático por `typedef` e aplicação explícita de extensão com prefixo | new_sali/backend (57 diagnósticos) | o backend |
| C3 | chave de constante que espalha uma `const` de outra biblioteca | new_sali/backend (pânico `ast.rs:55`) | o backend |
| C4 | literal de coleção grande e constante: quadro acima de 1 MiB, LLVM sem memória | bidi, unorm_dart (dargres), enough_convert ×3 | 5 dos primeiros 64 |
| C5 | `part 'package:…'` | diff_match_patch ×2 | 2 dos primeiros 64 |
| C6 | fuso horário no Windows pelo estado atual, não pelo ano do instante | equatable (exemplo) | 1 dos primeiros 64 |

(A tabela é refeita com o placar completo em §3.)

### C1. Emissão quadrática no programa grande

**Medido.** `compile-native bin/prod.dart --emit-ir` passou de 46 min
(18 min de CPU, até 1,9 GB) sem terminar. A amostragem da pilha (uma rodada
por minuto, 8 amostras cada) deu, em ordem: ~1 min de front-end; ~3 min de
lowering, quente em `sdk_fonte::implementacoes` (`EN/lower/sdk_fonte.rs:1005`);
~7 min nas passadas de `otimizar` (`efeitos::nao_lancam`, inlining,
escape); e daí em diante só `LlvmEmitter::emit_function`
(`EN/llvm/mod.rs:820`), com `memchr`/`memcmp` e `realloc`.

**As três buscas.**

1. `EN/llvm/mod.rs:820`: cada `CallStatic` procurava a função chamada com
   `module.functions.iter().find(|f| f.symbol == *symbol)` — linear no
   número de funções do módulo, comparando strings. Com N funções e C
   chamadas, O(N·C).
2. `sdk_fonte::implementacoes` e `implementacoes_por_classe`
   (`EN/lower/sdk_fonte.rs`), `classe_fechada`,
   `classe_fechada_por_modificador` e as implementações de um membro do
   programa (`EN/lower/membros.rs`, a busca por `cdecl`): a cada acesso a
   membro, uma varredura de TODAS as classes do programa (com a
   linearização de cada subtipo). O(acessos × classes).
3. `otimizar::efeitos::nao_lancam`: o maior ponto fixo por rodadas, cada
   uma varrendo o módulo inteiro, uma rodada por nível da cadeia de
   chamadas que lançam.

**Desenho.**

1. `LlvmEmitter::funcao_por_simbolo`: um `HashMap<&str, &Function>`
   montado uma vez em `LlvmEmitter::new`, com a PRIMEIRA função de cada
   símbolo (o que o `find` devolvia).
2. `Context::subtipos(cid)`: os subtipos de uma classe, calculados uma vez
   por classe (`memoria_subtipos`); as varreduras passam a percorrer só
   eles, na mesma ordem (a dos ids). `implementacoes` e
   `implementacoes_por_classe` guardam o resultado por (classe, nome)
   (`memoria_implementacoes*`). Tudo depende só do programa e das
   bibliotecas compiladas, fixos depois da construção do `Context`.
3. `nao_lancam` por lista de trabalho: as funções que lançam por si (com
   todas as do módulo supostas sem lançar) e, subindo pelas arestas
   inversas de `CallStatic`, as que chamam uma que lança. É o mesmo
   conjunto do ponto fixo (uma função lança sse alcança, por chamadas
   estáticas, uma operação que lança).

### C2. Membro estático por `typedef`; extensão explícita com prefixo

**Medido.** 57 diagnósticos `não suportado no backend nativo: elemento de
topo`: 28 em `postgres_fork/lib/src/substituter.dart`
(`case PostgreSQLDataType.text:` com `typedef PostgreSQLDataType =
PgDataType<Object>`), 28 em `binary_codec.dart` (o mesmo) e 1 em
`postgres/lib/src/v3/connection.dart:360`
(`async.StreamSinkExtensions(socket)`).

**Regra.** Especificação da linguagem, §15 "Type Aliases": um alias que
denota uma classe pode ser usado para acessar os membros estáticos dela
(`A.x`, `A.m()`, `A.x = v`, `A.values` de enum, tear-off `A.m`), e os
argumentos de tipo do alias não contam. §13.3 (extensões): `E(x).m` é
aplicação explícita, também com o nome qualificado por prefixo (`p.E(x)`).

**Desenho.** `FnBuilder::classe_do_alvo_estatico` (`EN/lower/chamadas.rs`)
devolve a classe de um alvo `C`, `p.C` ou `A` (alias cujo tipo-alvo é uma
interface); a leitura de propriedade (`EN/lower/expressoes.rs`, o ramo
`C.x`) e a atribuição (`EN/lower/atribuicao.rs`) passam a usá-la — a
chamada `A.m(…)` já aceitava o alias. Na chamada com alvo prefixado, o
elemento `Extension` faz o mesmo que `E(x)` sem prefixo: o valor é o
primeiro argumento (`EN/lower/chamadas.rs`).

### C3. Constante que espalha uma `const` de outra biblioteca

**Medido.** Pânico `index out of bounds: the len is 279 but the index is
811` em `crates/frontend/src/ast.rs:55`, pego pelo `catch_unwind` do
lowering, duas vezes no backend; num programa mínimo, sem pânico,
`identical(const [0, ...xs, 3], const [0, 1, 2, …, 3])` dava `false` (a VM
dá `true`).

**Regra.** Especificação §17.3 "Constants": duas expressões constantes com
o mesmo valor denotam o mesmo objeto; `...e` numa coleção constante
acrescenta os elementos de `e`.

**Desenho.** `partes_do_espalhado` (`EN/lower/constantes.rs`) lia o
inicializador da `const` (um `ExprId` da AST da unidade DELA) com a AST da
unidade corrente. Para uma `const` de outra unidade, a chave é calculada por
um `FnBuilder` daquela unidade, com a AST, a fonte e as resoluções dela.

### C4. Literal de coleção grande e constante

**Medido.** `bidi/test/bidi_test.dart`, `dargres/.../unorm_dart_test.dart`,
`enough_convert` (exemplo e `big5_test`): o executável morre com
`0xC00000FD` (estouro de pilha). `enough_convert/test/koi8/koi8_r_test.dart`:
`LLVM ERROR: out of memory` na compilação. O mínimo (um `const <int, Cat>{…}`
de 16 mil entradas) reproduz: o getter da constante tem 16 mil `alloca
[3 x i64]` (o buffer de argumentos do `[]=` por seletor, um por ponto de
chamada, todos no bloco de entrada) e, no `-O0`, uma palavra de
derramamento por valor que atravessa a conferência de exceção.

**Regra.** A pilha do isolado principal é a da thread principal do
processo: 1 MiB no Windows, a mesma reserva da VM
(`VM/runtime/vm/os_thread_win.cc:101-104`, `OSThread::GetMaxStackSize`).
A VM não estoura porque monta o literal num vetor preenchido por
`StoreIndexed` e chama um só `Map._fromLiteral` /
`_GrowableList._literal` (`VM/runtime/vm/compiler/frontend/kernel_binary_flowgraph.cc:4051-4140`;
`lib/_internal/vm_shared/lib/map_patch.dart:17`).

**Desenho** (`EN/lower/literais.rs`).

1. A tabela de dados (`preencher_de_tabela`, que já cobria strings, `bool`
   e `null`) passa a aceitar números, pelo tipo estático do literal: `i`
   (8 bytes) para `int`, `d` (os bits do `double`) para `double` — um
   literal inteiro num contexto `double` já tem tipo `double` na inferência.
   O runtime lê `d` em `dartforge_lista_de_tabela`
   (`RT/nativos_listas.rs`). As tabelas `Map<int, int>` viram dados.
2. O literal de 256 elementos ou mais que não cabe na tabela, com todo
   elemento constante (tem chave canônica, `chave_constante`), vai por
   funções auxiliares de 128 elementos (`<função>$lit<pos>_<i>(alvo)`),
   chamadas em ordem com conferência de exceção. Uma constante não lê
   locais, `this` nem parâmetros de tipo, então avaliá-la fora da função
   que contém o literal dá o mesmo valor, na mesma ordem; as `const` locais
   vão junto (`chaves_de_const_locais`), como no getter de constante. O
   literal de lista só de expressões grande e constante deixa o
   `AllocList` (um buffer de `n` palavras e `n` valores vivos no quadro)
   pelo mesmo caminho.

### C5. `part 'package:…'`

**Medido.** `diff_match_patch` (`lib/src/diff.dart:22`:
`part 'package:diff_match_patch/src/diff/utils.dart'`): `erro ao carregar o
programa: não foi possível ler …\lib\src\package:diff_match_patch\…`.

**Regra.** A URI de um `part` é resolvida como a de um `import` (§19.5
"URIs"): uma URI absoluta (`package:`) não é relativa à biblioteca.

**Desenho.** `crates/elements/src/load.rs`, ramo `DirectiveAction::Part`:
a URI `package:` vai direto ao `package_config`; só a relativa passa por
`resolver_relativo_a_package`.

### C6. Fuso horário no Windows

**Medido.** `equatable/example/main.dart`: `EquatableDateTime(2019, 1, 1, 1,
…)` na VM, `(2019, 1, 1, 0, …)` no nativo. No fuso de Brasília a VM dá
01:00 para `DateTime(2019)` (a meia-noite caiu no horário de verão daquele
ano) e o deslocamento de verão para todo instante de verão de anos
passados; o nativo usava `GetTimeZoneInformation` (o estado de AGORA) para
qualquer instante.

**Regra.** `VM/runtime/vm/os_win.cc`: `LocalTime` converte o instante com
as regras do fuso para o ano dele (`GetTimeZoneInformationForYear` +
`SystemTimeToTzSpecificLocalTime`, duas vezes: com o viés de verão e com
ele zerado; horas diferentes = verão); `GetTimeZoneOffsetInSeconds` é
`-_timezone` da CRT (depois do `_tzset`) menos o viés de verão atual
quando o instante está em verão (0 se a conversão falha);
`GetTimeZoneName` é o `DaylightName`/`StandardName` do fuso atual.

**Desenho.** `fuso_local` do Windows (`RT/nativos_sistema.rs`) faz o
mesmo, pelas mesmas funções; as bibliotecas de importação geradas ganham
`FileTimeToSystemTime`, `GetTimeZoneInformationForYear`,
`SystemTimeToTzSpecificLocalTime` (kernel32) e `_tzset`, `_get_timezone`
(ucrtbase) (`EN/ligador_windows.rs`).

### C7. O programa inteiro era baixado (sem mundo fechado)

**Medido.** Com C1 corrigido, a emissão do backend terminou: 311 850
funções e **2,8 GB** de LLVM IR. As maiores eram dados (as tabelas do
`unorm_dart`, 25 MB de IR cada; o `pg_timezone`; as gramáticas do
`highlight`) e ligações FFI geradas (`openssl_bindings`, 180 MB), quase
tudo inalcançável a partir do `main`. O `-O0` do Clang gasta ~44 bytes de
memória por byte de IR (24,8 MB de IR, pico de 1,08 GB, medido): 55 GB
para o módulo inteiro.

**Regra.** O Dart AOT só compila o que a análise de fluxo de tipos retém
(`VM/pkg/vm/lib/transformations/type_flow/`;
`VM/runtime/vm/compiler/aot/precompiler.cc`, `DropFunctions`).

**Desenho** (`EN/mundo_nativo.rs`). O mundo fechado do `crates/mundo` (o
RTA do JS de produção: classes instanciadas × seletores, o SDK como
fronteira) com as raízes do nativo: o `main`, o que tem
`@pragma('vm:entry-point')`, os nomes que o lowering chama por seletor
(`iterator`, `moveNext`, `current`, `[]=`…) e, como seletores externos,
**todo nome de membro escrito nas bibliotecas do SDK** (`x.nome`, `nome`
solto, `#nome`, campo de padrão de objeto) — superconjunto sintático do
que o SDK compilado pode chamar num objeto do programa. O que fica fora:

* função de topo, estática ou método com corpo (`FunctionRef::Function`)
  → corpo que lança `UnsupportedError` "código podado como inalcançável foi
  chamado: <símbolo>" (`FnBuilder::corpo_podado`; o símbolo continua
  definido);
* global → getter que lança (o setter fica); campo `late` de instância →
  getter que lança;
* membro de instância (inclusive acessor de campo) → sem entrada na tabela
  de métodos e sem adaptadores (`$c`, `$g`, `$s`, `$tc`…); o seletor fica
  tomado na linearização, para uma superclasse não preencher o lugar. Os
  membros sintéticos (encaminhadores de `noSuchMethod`) nunca são podados.

`DARTFORGE_SEM_PODA_DO_PROGRAMA=1` desliga. No backend: 38 062 de 110 817
funções do programa vivas; o IR caiu para 1,32 GB.

### C8. O inliner copiava getters preguiçosos de globais

**Medido.** Uma constante de 16 mil `CharacterCategory.lu` (o `bidi` do
`pdf_plus`) virou um getter de 97 MB de IR: cada leitura de valor de enum
recebia a cópia inteira do getter (alocação, registro como constante,
gravação do global), ~135 linhas por elemento.

**Regra.** A VM lê o campo estático e só chama o inicializador na primeira
vez (`LoadStaticField` + `InitStaticField`).

**Desenho.** `otimizar::inline::copiavel` recusa função com `StoreGlobal`
(o getter preguiçoso de global ou de valor de enum).

### C9. O módulo grande em partes; o endereço dos campos por chamada

**Medido.** Mesmo podado, o IR do backend tinha 1,3 GB: um Clang só não
cabe na máquina. O endereço dos campos de um objeto
(`emitir_endereco_dos_campos`, 15 instruções em linha) aparecia 465 mil
vezes — 21% das linhas.

**Desenho.**

1. `EN/particao.rs`: o texto do módulo é lido em itens (funções, globais,
   declarações, `comdat`) e dividido em partes de ~16 MB agrupadas pela
   biblioteca do símbolo. Itens com ligação externa ou `linkonce_odr` vão a
   uma parte só, e as outras recebem a declaração (`declare` com os tipos
   dos parâmetros; `@g = external global|constant <tipo>`); `private` e
   `internal` imutáveis (textos, vetores, os ajudantes `@df.*`) são
   copiados em cada parte que os cita; o único `internal global` mutável
   (`@df.area_id`) passa a externo na parte 0. Cada parte é montada só
   quando é a vez dela (`Plano::parte`) e tem a sua chave no cache de
   objetos — uma edição recompila só as partes que mudaram.
   `DARTFORGE_PARTE_MB` (0 desliga), `DARTFORGE_PARTES_PARALELAS` (padrão:
   metade das threads, no máximo 3). Vale também na produção: cada parte
   vira um bitcode e o ThinLTO do ligador as junta (`/opt:lldltojobs`,
   `--thinlto-jobs` no mesmo número).
2. `EN/driver.rs`, `gerar_partes`: as partes em paralelo; a ligação recebe
   todos os objetos.
3. No desenvolvimento (`-O0`), o endereço dos campos é `call @df.corpo(h,
   ctx)` (`llvm/mod.rs`, `ajudante_do_corpo`); a produção e os módulos do
   SDK continuam em linha (`alwaysinline`). O IR caiu de 1,32 para 1,06 GB.

### C10. Simplificação da HIR com trocas em cadeia

**Medido.** O LLVM recusava o IR: "use of undefined value '%v129'" no
`RIPEMD128Digest.processBlock` do `pointycastle` (`a = aa = state[0]` com
locais `int?`). Reproduz sem pacote (`corpus/nativo/124`), também com o
`dartforge` de antes destas mudanças.

**Desenho.** `otimizar::simplificar::dobrar_constantes` troca
`Unbox(Box(x))` por `x` e tira as duas instruções; quando o próprio `x`
também sai na mesma volta, a troca apontava para um valor removido. As
trocas agora seguem a cadeia antes de aplicar.

### C11. Tipos e tear-offs entre isolados

**Medido.** No backend compilado, o isolado do servidor morria com
`index out of bounds: the len is 1858 but the index is 1992`
(`RT/tipos.rs`, `tipo(id)`), e depois com `type 'Function' is not a
subtype of type '(Stream<dynamic>, dynamic) => Stream<dynamic>' in type
cast` (o `StreamIsolate` do projeto). Reproduzem em `corpus/nativo/125` e
`126`.

**Regra.** Na VM os argumentos de tipo e as closures atravessam
`Isolate.spawn` como objetos com o tipo.

**Desenho.**

1. Os ids de tipo do RTI eram por isolado; um id guardado como palavra
   crua (o `T` que uma closure genérica capturou no contexto, a tupla de
   tipos de uma chamada) chegava ao isolado novo como número e apontava
   para outro tipo. Agora a tabela canônica é do processo
   (`UniversoGlobal`, só de acréscimo, cada tipo vazado), e cada isolado
   guarda a cópia local dos que já viu (`Universo::obter`): só o tipo novo
   passa pela trava.
2. O tear-off canônico mandado numa mensagem era recriado no destino sem o
   corpo tipado, a ABI e o metadado RTI (`runtimeType` `Function`). O nó do
   grafo (`ValG::TearOff`) leva os três (`RT/portas.rs`).

### C12. `const []` padrão de parâmetro com variável de tipo

**Medido.** O servidor parava com `type 'List<dynamic>' is not a subtype
of type 'Iterable<(RequestContext<dynamic>, ResponseContext<dynamic>) =>
dynamic>' of 'middleware'` — o `{Iterable<T> middleware = const []}` do
`Router<T>` do `angel3_route`.

**Regra.** Uma constante não depende de variáveis de tipo: o contexto de um
literal `const` vale pelo fecho menor (a VM dá `List<Never>`,
`corpus/nativo/127`).

**Desenho.** `crates/types/src/inferencia/colecoes.rs`, `literal`: com
`const` explícito e sem argumentos de tipo, o contexto passa por
`bounds::least_closure` em relação às variáveis de tipo que aparecem nele.

### C13. Anotação de campo de struct por alias ou prefixo

**Medido.** O `/metrics` do backend devolvia 500: "struct or union not
registered in the DartForge native backend" — as estruturas do Win32 do
`prometheus_client` do projeto anotam os campos por aliases
(`typedef DWORD = Uint32;` … `@DWORD()`).

**Desenho.** `EN/lower/ffi.rs`, `tipo_do_campo`: a classe da anotação vem
do escopo da biblioteca (`classe_da_anotacao`: `@Nome()`, `@p.Nome()`,
alias de classe), não do texto (`corpus/nativo/128`).

### C14. Testes do SDK sem mensagem no macOS

**Medido.** No job do macOS, `producao_e_um_executavel_autocontido` e
`poda_tira_membros_nao_usados` falhavam sem mensagem nenhuma; com
`--test-threads 1` passavam. O teste de medição
`medir_lowering_das_bibliotecas_da_fonte` trocava o gancho de pânico do
PROCESSO por um silencioso e não o restaurava: todo pânico dos testes que
rodavam junto sumia. O gancho saiu (`EN/sdk_modulo.rs`); e a ligação de
produção passou a limitar as tarefas do ThinLTO (C9), o que reduz o pico
de memória de dois testes de produção simultâneos.

## 2. Programas do corpus

| programa | causa |
|---|---|
| `corpus/nativo/120_constantes_entre_bibliotecas/` | C3 |
| `corpus/nativo/121_estaticos_por_typedef_e_extensao_prefixada/` | C2 |
| `corpus/nativo/122_literais_grandes_constantes.dart` | C4 |
| `corpus/nativo/123_fuso_horario_por_ano.dart` | C6 |
| `corpus/nativo/124_atribuicao_encadeada_anulavel.dart` | C10 |
| `corpus/nativo/125_tipos_capturados_entre_isolados.dart` | C11 |
| `corpus/nativo/126_tearoff_entre_isolados.dart` | C11 |
| `corpus/nativo/127_const_padrao_com_variavel_de_tipo.dart` | C12 |
| `corpus/nativo/128_ffi_anotacoes_por_alias.dart` | C13 |

C1, C7, C8 e C9 são de escala (medidos no backend; o 122 cobre a parte de
C4 que cabe no corpus); C5 precisa de `package_config` e vai num teste do
carregador (`crates/elements/tests/unit_tests.rs`, `parte_por_uri_package`).

## 3. Resultados

(preenchido na validação)
