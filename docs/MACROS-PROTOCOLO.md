# Macros: hospedeiro, executor nativo e o serviço `macro.*` do `dfexec/1`

Contrato dos passos P7–P11 do plano "Dart 3.7–3.13 e macros". **Estado
(2026-09-23, rodada de macros)**: augmentations (P7), a API de macros
reescrita (P8, lado Dart) e o hospedeiro (P9, lado Rust) implementados; o
executor **nativo** espera o backend nativo compilar o que a API usa (§3).
Placar em §8. **Substitui** `docs/historico/MACROS-ARQUITETURA.md` §"herdar as
fases, descartar o transporte" e a Q4 (D8). Decisões de base em
[`VERSOES-LINGUAGEM.md`](VERSOES-LINGUAGEM.md) §0; compatibilidade com a
toolchain oficial em [`MACROS-COMPATIBILIDADE.md`](MACROS-COMPATIBILIDADE.md).

## 1. O que se implementa

* **API e comportamento observável**: o da 1ª geração que o SDK 3.6.2 executa
  com `--enable-experiment=macros` (D5) — `package:macros` 0.1.3-main.0 /
  `_macros` 0.3.3. Caso de aceite: `@JsonCodable` do `package:json` 0.20.4.
  **A API é reescrita por nós** (D3) em `pacotes/macros` (pacote `macros`,
  mesma superfície pública), seguindo
  `references/dart-language/working/macros/feature-specification.md` e usando
  o `_macros` 0.3.3 só como referência de forma; não vendorizamos `_macros`.
  O `json.dart` do `package:json` 0.20.4, **sem mudança**, passa no analyzer
  oficial 3.6.2 contra ela com zero problemas.
* **Oráculo**: a VM 3.6.2 (saída do programa) e o texto de augmentation que o
  CFE 3.6.2 gera, **byte a byte**. O texto é o que o CFE grava na tabela de
  fontes do `.dill` (`buildMergedAugmentationLibraries`, URI
  `dart-macro+<biblioteca>`), extraído por `scripts/oraculo_augmentation.dart`
  (`dart compile kernel --enable-experiment=macros` e depois o script).
* **Divergências spec × CFE 3.6.2** (vale o CFE, D9), todas medidas:
  * sem embrulho de corpo nem `augmented()`;
  * sem `evaluate(Code)`/`DartObject` nem `Resource` (espaço reservado no
    protocolo); aspect macros fora;
  * **listas de introspecção na ordem de declaração**, não lexicográfica
    (`:720-733`): o `fieldsOf` do CFE percorre os membros em ordem de fonte, e
    o texto do `@JsonCodable` sai nessa ordem (`nome, idade, apelido, notas`);
    ordenar mudaria o texto;
  * o que o CFE 3.6.2 não implementa, o hospedeiro também recusa com erro de
    implementação (`topLevelDeclarationsOf`, `valuesOf`), e o que ele
    simplifica, o hospedeiro reproduz (`hasBody` sempre verdadeiro, metadados
    vazios, sem parâmetros de tipo em métodos e construtores, retorno de
    construtor omitido, `typesOf` só com classes, `resolveIdentifier` só com
    membros locais da biblioteca);
  * ordem de execução da fase 1 e 3: a da diretiva `library` primeiro (a spec
    diz depois) — §4.

## 2. Regra governante (PLANO.md, 2026-09-23) — como este desenho a cumpre

1. **Custo zero para quem não usa.** O outline anota as classes `macro`
   (`Program::classes_macro`) na mesma passada que cria os elementos; sem
   nenhuma, o `compile-js` nem chama o hospedeiro (`macros_host::tem_macros`,
   um `is_empty`), e `aplicacoes::detectar` volta na primeira linha. Sem
   `--enable-experiment=macros`, `macro class` nem é aceita. O carregador só
   procura `x.macro.dart` quando há fontes geradas em memória. Portão
   estrutural: `macros_host::sessoes() == 0` num programa sem macro (teste
   `sem_macro_nao_abre_sessao`), além do portão de tempo do corpus.
2. **Uma infraestrutura só**: o protocolo é o `dfexec/1` dos builders
   (docs/BUILD-PROTOCOLO.md), com as macros como o serviço `macro.*` (§5). O
   mesmo enquadramento, o mesmo handshake, o mesmo processo executor.
3. **Executor persistente e quente**: um processo por sessão, iniciado só se
   há aplicação; as instâncias são criadas uma vez por (macro, construtor,
   argumentos) e servem às três fases.
4. **Só reexecuta o que mudou**: cada execução registra as consultas feitas
   (`Consultor::registro`); o cache por digest (§6) é o próximo passo, junto
   do executor nativo e do `dartforge dev` (P10).
5. **Paralelo onde a spec permite**: o protocolo já numera pedidos e
   consultas por execução (várias execuções em voo são possíveis); a sessão
   ainda executa uma por vez.
6. **Nada de contabilidade em disco**: o texto vive em memória
   (`<biblioteca>.macro.dart` em `elements/src/gerado.rs`); o disco só recebe
   o que o usuário pede (`dartforge macros --materializar`).
7. **Orçamentos medidos**: entram quando houver executor (§8).

## 3. Quem executa: o executor nativo (D4)

A macro é Dart do usuário. Ela é compilada pelo **nosso** `emit_native` e
executada pelo **nosso** executor — o JIT ORCv2 sobre o mesmo IR
(`crates/jit`) ou o AOT — num **processo separado** (sandbox da spec,
`:1226-1234, :1302-1395`), falando `dfexec/1` por stdio. Não há Node, motor
JS embutido nem runtime de terceiros. `int` tem a semântica da VM (64 bits).

**Estado**: o trait `executor::ExecutorMacros` tem três implementações:

| implementação | uso |
|---|---|
| `Indisponivel` | diagnóstico quando nenhum executor foi configurado; não é o caminho normal do JS com macro |
| `ExecutorDfexec<C: Canal>` | o cliente `macro.*` sobre qualquer canal: processo (`CanalDeProcesso`), gravação (`CanalGravador`) ou sessão gravada (`CanalGravado`, o executor falso dos testes) |
| `vm::iniciar` | executor de materialização e caminho provisório de `compile-js`/`jsprod`: executa a nossa API numa VM Dart (MACROS-COMPATIBILIDADE.md) e recarrega a augmentation em memória |

**O que espera o executor nativo**: compilar `pacotes/macros` (a API, o
`executor/servico.dart` e o `canal_stdio.dart`) e a biblioteca da macro com o
*bootstrap* gerado (`vm::bootstrap`: mapa `uri#Classe` → construtor por
*tear-off* e `Function.apply`), com `async`/`Future`, `Completer`, coleções,
records, `switch` de padrões, `dart:convert` (JSON) e `dart:io` (stdio). O
ponto de encaixe é um `ExecutorDfexec<CanalDeProcesso>` com o executável
nativo no lugar do `dart`.

Bibliotecas permitidas no fecho de uma macro (`:1329-1350`):
`dart:{async,collection,convert,core,math,typed_data}`; o executor em si usa
`dart:io` (stdio), fora do fecho da macro.

## 4. Fases e ordem

Implementação: `crates/macros_host/src/{aplicacoes,sessao}.rs`.

* **Detecção**: uma anotação é aplicação se resolve para um construtor de
  classe `macro` (`@M()`, `@M.nome(…)`, `@p.M(…)`). Argumentos: literais,
  coleções de literais e, o resto, `Code` com o texto da expressão.
* **Ordem na declaração**: da direita para a esquerda; membros (campos e
  métodos, depois construtores) antes do tipo.
* **Ordem entre declarações** (a do CFE 3.6.2, `kernel_macro_macro.dart`):
  * tipos e definições, por biblioteca: diretiva `library`, funções e
    variáveis de topo, e, por classe na ordem do fonte, membros e classe;
  * declarações: as classes na ordem da hierarquia (supertipos antes), e só
    então as de biblioteca e de topo.
* **Fases e recarga**: depois da fase 1 o texto parcial entra no programa;
  na fase 2 **cada** resultado não vazio entra antes da próxima aplicação (o
  CFE cria uma biblioteca de augmentation por resultado); depois da fase 3, o
  texto final. A recarga é a carga comum do `elements` com a geração em
  memória, e a augmentation entra como `UnitRole::Augmentation` depois das
  partes (`load.rs`, `anexar_augmentation_de_macro`).
* **Identificadores estáveis**: o id de um identificador é o de uma
  `modelo::Chave` textual (biblioteca, dono, nome, tipo), resolvida de novo
  no programa corrente a cada uso — os ids do `elements` mudam a cada recarga,
  e a Regra 1 funde por identificador.
* **Texto** (`montagem.rs`, `:288-487`): as declarações de topo na ordem dos
  resultados; tudo o que aumenta um tipo fundido num `augment <tipo>` (Regra
  1), tipos na ordem de primeira aparição por categoria; identificador vira
  `prefixN.Nome` (import com prefixo na ordem de primeiro uso; o nome-base é o
  primeiro de `prefix`, `prefix0`, … que não aparece no texto), `this.` para
  membro de instância sem receptor, `prefixN.Classe.nome` para estático e
  construtor. Cabeçalho `augment library '<uri>';` (a forma do CFE; a
  materialização troca pela da versão do SDK). A Regra 3 da spec (linha em
  branco) **não** é a do CFE: ele separa com `\n` só, e vale o CFE.
* **Erros**: diagnóstico da macro (`Severity.error`), exceção de
  implementação e falha do executor viram erro **na anotação**
  (`<unidade>: aplicação de @Macro: …`); aviso e informação vão para a saída.

## 5. O serviço `macro.*`

Transporte e handshake: os do `dfexec/1` (BUILD-PROTOCOLO.md §1–§2), com
`"servicos": ["macro"]`. O executor responde ao `ola` com as macros do
bootstrap (`macros: [{macro: "uri#Classe", construtores: [...]}]`).

Hospedeiro → executor (numerados pelo hospedeiro):

* `macro.instanciar {id, macro, construtor, argumentos: {posicionais,
  nomeados}}` → `macro.instancia {id, instancia, interfaces}` — `interfaces`
  são os nomes das interfaces de macro que a instância implementa
  (`ClassDeclarationsMacro`…), com que o hospedeiro decide as fases.
  Argumento: `{t: null|bool|int|double|string|lista|set|mapa|codigo|tipo,
  v}` (`int` em texto, para os 64 bits);
* `macro.executar {id, instancia, fase: tipos|declaracoes|definicoes, alvo,
  modelo}` → `macro.resultado {id, resultado}`; o `modelo` traz as
  bibliotecas e, para classe, a **pré-busca** dos membros
  (`membros: {"<id>": {campos, metodos, construtores}}`): o `@JsonCodable` não
  faz ida e volta para eles;
* `macro.descartar {instancia}`; `fim`.

Executor → hospedeiro, durante uma execução: `macro.consulta {id, execucao,
tipo, args}` → `macro.resposta {id, valor}` ou `{id, erro: {tipo:
implementacao|ciclo|inesperado, mensagem}}` (vira a `MacroException`
correspondente na macro). Consultas: `resolverIdentificador {uri, nome}`,
`declaracao {ident}`, `membros {dono, tipo}`, `tiposDe {biblioteca}`,
`declaracoesDe`, `resolver {tipo}` → `{chave, declaracao, args}`,
`ehExatamente {a, b}`, `ehSubtipo {a, b}`, `comoInstanciaDe`, `inferirTipo
{chave}`. `inferType`, `asInstanceOf` e subtipo com argumentos de tipo
respondem erro claro: dependem do `crates/types` (pedido ao dono).
Reservados: `avaliar {codigo}` (`evaluate`) e `recurso {uri}` (`Resource`).

**Modelo** (JSON): identificador `{id, nome}`; tipo `{t: nomeado, ident,
args, anulavel}` | `{t: funcao, …}` | `{t: record, …}` | `{t: omitido,
chave}`; declaração `{k: classe|mixin|enum|extensionType|typedef|tparam|
metodo|construtor|campo|funcao|variavel|parametro|valorEnum, ident, lib, …}`
com os campos da API (`abstract`, `final`, `superclasse`, `interfaces`,
`posicionais`, `nomeados`, `retorno`, `estilo`…); biblioteca `{k:
biblioteca, id, uri, versao}`.

**Resultado**: `{diagnosticos, excecao, valoresDeEnum, extends, interfaces,
biblioteca, mixins, tiposNovos, tipos}`, com os mapas do
`MacroExecutionResult` como **listas de pares** `[id do tipo, [código…]]`
(a ordem de inserção é a da montagem). Código: `{k: CodeKind, p: [partes]}`,
parte = texto, `{i, n}` (identificador), `{o}` (tipo omitido) ou código.
O executor devolve o código **estruturado**; o texto é montado no hospedeiro
(§4), que precisa dele para a recarga de qualquer jeito, e onde a resolução
dos identificadores é local. As declarações `augment` que os builders da fase
3 formam (`augmentacaoDeFuncao`, `augmentacoesDeVariavel` em
`executor/resultado.dart`) têm o texto do CFE 3.6.2.

## 6. Cache e incrementalidade (próximo passo)

* **Chave de uma aplicação** = hash(identidade do bundle da macro,
  argumentos, digest do alvo no modelo). **Valor** = resultado estruturado +
  as consultas feitas (`Consultor::registro`), com o digest de cada
  resposta. Tudo igual na reanálise → reutiliza **sem falar com o executor**.
* Edição de corpo não altera o modelo (só declarações): o caminho de 227 ms
  continua com zero execuções (`macros_executadas` no relatório).
* Entra com o executor nativo e a integração do `dartforge dev` (P10); hoje
  o `dev` não aplica macros.

## 7. Corpus de macros

`corpus/macros/`, job `macros` do `pesado.yml` (VM e DDC 3.6.2 com
`--enable-experiment=macros`, e 3.13.4 para a forma `part of`):

* `400`–`405` — augmentations escritas à mão (AUGMENTATIONS.md §5);
* `410_json_codable` — pacote com `@JsonCodable`, `@JsonEncodable` e
  `@JsonDecodable` (aninhado, anulável, `List`/`Set`/`Map`, `DateTime`,
  `bool`, `num`); `esperado/modelos.augmentation.dart` é o texto do CFE 3.6.2
  e `esperado/sessao.dfexec` a sessão gravada do protocolo. O job de macros
  executa a fonte original nos oráculos e uma cópia com `.macro.dart`
  materializado pelo `build_runner` nos backends DartForge.
* `411_pedido_independente` — `@JsonCodable` em uma biblioteca nova, fonte
  anotada original nos quatro executores; o JS roda a macro automaticamente
  com o executor VM provisório, sem `dartforge-entrada.txt` nem preparação
  manual. O `.macro.dart` do builder também é comparado byte a byte ao CFE.
* `412_argumento_posicional` — macro própria `@Rotulo('P7')`, exercitando
  `Function.apply` e o argumento posicional do protocolo no caminho direto
  de desenvolvimento e produção. A augmentation do hospedeiro é comparada
  byte a byte com a fonte extraída do kernel pelo CFE 3.6.2.
* `413_argumentos_nomeados` — macro própria `@Etiquetas(prefixo: 'A')`,
  exercitando argumento nomeado em desenvolvimento e produção. O texto
  gerado pelo hospedeiro é comparado byte a byte com o CFE 3.6.2.
* `414_funcao_topo` — macro em função de topo (`FunctionDeclarationsMacro`),
  exercitando o modelo da função e declaração nova na biblioteca. O programa
  chama a função gerada e a augmentation é comparada byte a byte ao CFE.
* `415_variavel_topo` — macro em variável de topo (`VariableDeclarationsMacro`),
  exercitando o modelo da variável e uma função gerada que acessa seu valor.
  O programa chama essa função e o texto é comparado byte a byte ao CFE.
* `416_definicao_funcao` — `FunctionDefinitionMacro` substitui o corpo de
  uma função de topo na fase 3. A VM executa o corpo aumentado e o texto
  estruturado pelo hospedeiro é comparado byte a byte ao CFE 3.6.2.

## 8. Placar (medido nesta rodada)

* augmentations: 6/6 do `corpus/macros/40*` batem com o oráculo (3.6.2 e
  3.13.4) no perfil de desenvolvimento;
* API: `json.dart` do `package:json` 0.20.4 analisado contra `pacotes/macros`
  pelo analyzer 3.6.2: 0 problemas;
* hospedeiro: augmentation montada **igual à do CFE 3.6.2, byte a byte**, no
  `410_json_codable` (4 classes, 3 macros, 62 linhas) — pela VM (executor de
  materialização, teste `vm_executa_a_macro_e_bate_com_o_cfe`) e pela sessão
  gravada sem executor (teste `sessao_gravada_reproduz_o_texto_do_cfe`) — e
  no teste unitário da montagem (`montagem::testes`, o `Usuario` do 402);
* materialização: o `410` materializado compila no `dartforge compile-js` e
  no `dartforge-jsprod` sem executor e imprime o mesmo que a VM 3.6.2 com as macros
  (MACROS-COMPATIBILIDADE.md §3);
* custo zero: `sem_macro_nao_abre_sessao` (0 sessões, 0 recargas, executor
  nunca tocado).

**Pendente** (registrado, não silencioso): o grafo de espera da fase 2 com
`MacroIntrospectionCycleException` (hoje a fase 2 roda na ordem do CFE, sem
espera), os erros de sombreamento (`:960-965`) e de macro aplicada no próprio
SCC (`elements/src/ciclos.rs`), `inferType`/subtipo genérico (dependem do
`crates/types`), o cache por digest (§6), a execução paralela na fase, o
`dartforge dev`/LSP (P10) e o executor nativo (P11).
