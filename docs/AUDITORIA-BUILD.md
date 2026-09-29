# Auditoria do build independente — acompanhamento

Itens `DF-BUILD-NNN` da auditoria de 2026-09-28 (revisão `95b17006`): o que
foi feito, o que foi refutado pelo código oficial e o que segue aberto.
O código oficial de referência é o do `build_runner_core` 8.0.0 e do
`build_config` do pub cache.

| Item | Estado | Nota |
|---|---|---|
| DF-BUILD-001 descobrir builders sem `build_runner` | feito | `dartforge_build::detectar`: algum pacote resolvido com `builders:`/`post_process_builders:` no `build.yaml`, ou o `build_runner` resolvido. |
| DF-BUILD-005 não mascarar falha com resultado antigo | feito (CLI) | `Origem::Falha`: o builder que lança, registra erro severo, escreve saída não permitida ou derruba o executor no meio da ação não cai mais no apoio; a ação fica sem saídas, `RelMotor::falhas` a lista e `dartforge build` falha. Só a ausência de executor (`FalhaDart::Indisponivel`) segue a política de apoio. Falta o `dev` anunciar a falha em vez de republicar. |
| DF-BUILD-006 modo independente | parcial | `dartforge build` conta a ação de apoio que entrega conteúdo lido do disco do `build_runner` (`RelMotor::apoio_com_saida`) como saída sem produtor do DartForge e falha sem `--aceitar-pendentes`. Aberto: as ações de apoio **sem** conteúdo — o oficial não escreveu nada ali, e o DartForge não sabe por conta própria que não escreveria: no `limitless_ui/example`, 541 do `ngdart` (bibliotecas do `ngcompiler`, que o nativo recusa) e 5 parciais do Sass; hoje só aparecem no relatório (`apoio`). |
| DF-BUILD-007 código de saída | feito (CLI) | Pendentes ou apoio: falha sem `--aceitar-pendentes`; no `--comparar`, depois do placar. |
| DF-BUILD-009 geração no nativo e no JIT | feito | `CompileOptions::gerador` (o mesmo contrato do `compile-js`): o `compile-native`, o `aot` e o `run`/`reload` (JIT) ligam o motor do projeto (`motor::com_gerador`); a carga tolerante sem os gerados serve de resolvedor e a geração volta em memória. `crates/cli/tests/gerado_nativo.rs`: `corpus/builders/i18n` (que importa `.i18n.dart` inexistentes no disco) compila e roda por AOT e JIT com a saída da VM. Erro do motor agora falha também o `compile-js` (antes virava geração vazia e leitura do disco). |
| DF-BUILD-010 reler `package_config` | feito | Ao refazer o motor por mudança de configuração, o arquivo é relido do disco. |
| DF-BUILD-012 builder de validação sem saídas | refutado | No `build_runner_core` 8.0.0 um builder só roda para entradas com nó gerado na fase (`BuildImpl._matchingPrimaryInputs` percorre `_assetGraph.outputsForPhase`): entrada sem saída esperada não executa. `Acao::viva` (sem saídas = sem ação) é o comportamento oficial. |
| DF-BUILD-013 demanda de builders opcionais | parcial | O placar separa a saída opcional que o grafo oficial não pede (`Placar::nao_solicitados`), sem excluir por nome. A execução preguiçosa (fases opcionais só quando lidas ou quando produzem entrada primária de fase posterior — `_runPhases` pula `isOptional`, `_matchingPrimaryInputs`/`_isReadableNode` as disparam) segue aberta. |
| DF-BUILD-017 `build.yaml` ilegível | feito | Só a ausência vale o padrão; erro de leitura é erro. |
| DF-BUILD-018 configuração fora da raiz | parcial | O `build.yaml` de cada pacote (exista ou não) e os overrides `<pacote>.build.yaml` da raiz, com o próprio diretório raiz (a marca muda quando um override aparece), entram na configuração observada: mudou, o motor é refeito relendo o `package_config.json`. Teste `configuracao_observada.rs`. Falta a prova "incremental igual ao limpo" numa sessão com edição de `build.yaml` de dependência. |
| DF-BUILD-020 wrapper Sass do ngcomponents | feito | `crate::equivalente`: a fábrica que só devolve `SassBuilder(..)` literal roda pelo Sass nativo. O ngcomponents trava o `sass` em 1.66.0: modo compatível com o 1.66 escolhido pelo lock (`9d097afb`); ngcomponents 735 iguais / 0 pendentes / 0 diferentes, os 70 `.scss.css` inclusive. |
| DF-BUILD-022 identidade das substituições nativas | feito | O gerador nativo só substitui o builder quando o pacote imitado vem do pub.dev (`source: hosted`, `description.url` do pub.dev) na versão imitada (`Travado::do_pub_dev`); `path`, `git` ou outro servidor com a mesma versão vão ao executor. |
| DF-BUILD-026 comparação bidirecional | parcial | Saída a mais aparece como `diferente`, ou como `não solicitado` se o builder é opcional. |

Os demais (002, 003, 004, 008, 009, 011, 014, 015, 016, 018, 019, 021–025,
027–031) seguem abertos; ver a auditoria.
