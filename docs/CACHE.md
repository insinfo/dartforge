# Cache de saída da sessão

`CompilerSession` mantém a última compilação bem-sucedida, com opção de otimização e
snapshot integral do grafo. `compile_path` retorna `Compilation { javascript: Arc<str>,
stats: SessionStats }`. A sessão não é um compilador incremental: qualquer diferença
recompila todas as unidades; não há cache de tokens, AST, tipos ou IR.

## Correção e invalidação

Cada chamada usa novamente `dartforge_packages::load`, incluindo a resolução atual de
`package_config.json`. A comparação usa igualdade exata do grafo: entrada, caminhos
canônicos, conteúdo das fontes, imports, exports, destinos, spans e combinadores.
A opção de otimização também faz parte da chave. Não se usa mtime, tamanho de arquivo
ou hash como prova de igualdade.

Remapear um pacote para outro caminho invalida mesmo quando as fontes são idênticas.
Uma configuração reescrita que produz exatamente o mesmo grafo e opções resolvidas
pode reutilizar a saída. Não há cache persistente entre versões do compilador.

Falhas de leitura, configuração, sintaxe ou semântica descartam a entrada armazenada e
retornam erro; saída antiga nunca é devolvida como resultado da solicitação que falhou.
Após recuperação, a próxima chamada recompila. O carregamento não é um snapshot atômico
do filesystem: arquivos alterados durante a leitura podem ser observados em instantes
diferentes. Editores/watchers devem agendar outra solicitação após novas alterações.

## Contagens e memória

- `cache_hit`: grafo e opção coincidem com a entrada armazenada.
- `source_units_loaded`: fontes carregadas nesta chamada, inclusive em acertos. Não é
  contagem de operações de disco; exclui leituras de configuração e metadados.
- `compiled_units`: zero no acerto; todas as unidades enviadas ao pipeline no miss.

A política armazena no máximo uma entrada, com limite padrão de **16 MiB de payload**:
fontes, caminhos, URIs, nomes de combinadores e JavaScript. Exclui capacidade ociosa,
estruturas, overhead do alocador e controle de Arc; não é um limite de RSS. Compilações
maiores funcionam, mas não são retidas. `with_cache_limit_bytes(0)` desativa retenção.
A memória transitória da compilação e saídas Arc guardadas pelo chamador não são
controladas pelo orçamento. `clear()` descarta a entrada da sessão.

## Referências consultadas e decisões

- Rolldown, revisão `a799ae4b6b9c2faf2eeac94cfa6d053dd6aed211`:
  [ScanStageCache](https://github.com/rolldown/rolldown/blob/a799ae4b6b9c2faf2eeac94cfa6d053dd6aed211/crates/rolldown/src/types/scan_stage_cache.rs)
  mantém snapshot, importadores e revisitas após falhas. Inspira a exigência de manter
  o estado coerente; nesta etapa simplificamos para descartar a entrada após erro.
- Dart SDK, revisão `6c4009687d2c881f880127fc12b9ad7c72111d33` (clone moderno, **não Dart 3.6.2**):
  [incremental_compiler.dart](https://github.com/dart-lang/sdk/blob/6c4009687d2c881f880127fc12b9ad7c72111d33/pkg/front_end/lib/src/base/incremental_compiler.dart)
  mantém URIs invalidadas, trata mudanças de pacotes e compara fontes. Serve apenas
  como referência arquitetural; não copiamos o compilador incremental nem usamos
  essa revisão como contrato de linguagem do alvo 3.6.2.

- Dart SDK **3.6.2**, tag fixada em `b0cc5495e0f5e8ae150825a5352e708cb49e65ff`:
  [incremental_compiler.dart](https://github.com/dart-lang/sdk/blob/b0cc5495e0f5e8ae150825a5352e708cb49e65ff/pkg/front_end/lib/src/base/incremental_compiler.dart)
  registra invalidação por URI e trata atualização da configuração de pacotes como
  motivo para invalidação ampla. A consulta foi feita com `git show 3.6.2:...`, sem
  alterar o checkout moderno de referência. Recarregar configuração e comparar o
  grafo completo é a estratégia conservadora desta primeira sessão.

Antes de granularidade incremental, será necessário modelar dependências por símbolo,
invalidar assinaturas e dependentes, preservar diagnósticos e estabelecer equivalência
com recompilação limpa. Esta implementação não afirma ganho de desempenho sem medição.

## Verificação

    cargo test -p dartforge-compiler --lib session::tests

O corpus inclui acerto com Arc compartilhado, dependência alterada com mesmo tamanho e
mtime restaurado, remoção de arquivo, mudança de aresta/opção, remapeamento de pacote,
recuperação após erro e orçamento sem retenção.
