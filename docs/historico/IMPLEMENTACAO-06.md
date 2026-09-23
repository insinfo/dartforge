# Incremento 06 — Pacotes, namespaces exportados e sessão

Alvo: Dart 3.6.2; código original MIT e documentação Rust em português.

## Comportamento implementado

- `package:` com descoberta ascendente de `.dart_tool/package_config.json` v2,
  resolução de URIs e percent-encoding, nomes e configurações inválidas diagnosticados.
  `packageUri` omitida usa a raiz do pacote, conforme a especificação.
- Filtros `show`/`hide` sequenciais e exports transitivos com ciclos. A identidade da
  declaração elimina falsas ambiguidades em diamantes; declarações locais prevalecem.
  Privacidade e namespaces importados/exportados permanecem distintos.
- `CompilerSession` guarda a última saída em `Arc<str>` com limite de payload.
  Toda solicitação recarrega as fontes e a configuração; comparação exata evita
  depender de tamanho, mtime ou colisão de hash. Erros descartam a entrada anterior.
- O comando `graph` agora exibe exports e combinadores.
- A validação de versão `// @dart` respeita comentários aninhados e o preâmbulo,
  tanto na API em memória quanto na compilação por arquivo.

## Decisões de desempenho

Exports usam uma fila de dependentes, propagando apenas namespaces alterados, com
conjuntos de origens declarativas que convergem mesmo em ciclos. Imports aproveitam
ordenação do BTreeMap sem alocação e ordenação intermediárias. A validação de raízes
converte URIs em caminhos uma vez por pacote, evitando conversões dentro de cada par.

O benchmark de bibliotecas separa carga, compilação pré-carregada, pipeline completo,
cache exato e miss após limpeza. Um hit continua incluindo leitura e descoberta do
grafo; qualquer alteração recompila todas as unidades. Não é compilação incremental.

## Referências e alcance

[Referências verificadas](REFERENCIAS-06.md), [pacotes](PACKAGES.md),
[bibliotecas](MODULES.md) e [cache](CACHE.md) registram contratos e limites.
O clone do SDK mantém seu checkout moderno, mas a tag 3.6.2 foi obtida e consultada
explicitamente no commit `b0cc5495e0f5e8ae150825a5352e708cb49e65ff`.

Ainda faltam prefixos, parts, runtime dart:, extensions entre bibliotecas, generics,
construtores explícitos e ngdart completo. Versões de linguagem diferentes de 3.6
são diagnosticadas. Nenhuma medição deste incremento prova vantagem sobre DDC/dart2js.

## Validação concluída

- 168 testes Rust aprovados, incluindo doctests e todos os testes de execução em Node.
- Formatação, Clippy com warnings proibidos, rustdoc e build release aprovados.
- 25 casos diferenciais por modo: 50 comparações aprovadas com Dart 3.6.2/dart2js O2.
  Incluem package_config portátil, classes reexportadas e ciclos de exports.
- Relatórios: [direto](dados/conformance-increment-06.json),
  [constantes](dados/conformance-increment-06-constants.json) e
  [benchmark bruto](dados/benchmarks/increment-06-libraries.json).

Corpus sintético: 101 arquivos, 21 amostras por fase, filesystem aquecido.
Medianas observadas nesta máquina (ms):

| Fase | Mediana ms |
| --- | ---: |
| load | 9.992 |
| link_preloaded_graph | 0.753 |
| pipeline | 11.158 |
| pipeline_constants | 9.572 |
| session_exact_hit | 7.388 |
| session_cleared_miss | 9.655 |

Fases são medidas separadamente, portanto suas medianas não são aditivas.
O custo de leitura continua presente no cache. Não extrapolar este corpus para ngdart
ou comparações com DDC/dart2js. Estado Git e amostras estão no JSON bruto.
