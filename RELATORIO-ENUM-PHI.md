# Fatos de enums através de phis e caches — 19/10/2026

A regressão real de enums simples falhava porque a análise exigia que o bloco
de sucesso do getter dominasse o bloco inteiro de um `phi`. Na execução, cada
entrada do `phi` é usada em sua aresta: o caminho frio chega depois do check de
exceção, enquanto o quente lê o cache já inicializado. O bloco de sucesso frio
não domina a junção desses dois caminhos, mas isso não invalida a entrada fria.

A análise agora verifica usos de `phi` nas arestas. Uma leitura quente só recebe
um token quando a guarda de inicialização do seu próprio cache foi reconhecida;
o cache e seus escritores continuam auditados pelo inventário. Phis só propagam
tokens idênticos com todos os predecessores conferidos. Não se introduzem fatos
iniciais em ciclos; identidades, definições SSA e disponibilidade são verificadas.

O check também cobre a aresta direta para um phi no bloco de sucesso, trabalho
adicional no bloco quente e reordenação física dos blocos. Escritas/exposição
introduzidas numa cópia especializada invalidam sua análise. O getter, leitura
de cache, parâmetros reais e checks de exceção continuam sendo executados; só
comparações de identidade Ref comprovadas podem ser substituídas.

## Evidências

- A regressão foi reproduzida antes da edição. Depois, sete testes de enums
  passaram. Há 12 variantes negativas de guardas/phis, além das 11 anteriores de
  escritores, aliases, forma do getter, marcadores e exposição.
- A fixture real agora certifica os dois getters simples e dobra a comparação
  existente. Enums com forma aprimorada continuam sem essa premissa.
- Suíte nativa completa: **769 passaram, 2 falharam, 9 ignorados**. Resolveram-se
  a falha de enums e quatro novas regressões foram adicionadas. As falhas de
  preparação ARC do notifier e especialização do kernel de cor permanecem,
  exatamente como na base anterior; não surgiu falha nova.
- Um produtor congelado compilou a fixture existente de identidade de padrões
  enum (inclui bibliotecas homônimas, aliases, enums aprimorados, igualdade com
  efeitos e exceção) e o kernel comum de cores da dart_ui. Saídas OFF/ON da
  especialização constante e sob GC stress foram idênticas às da VM e do AOT
  oficiais do Dart 3.6.2. O kernel de cor consumiu 2.736 linhas por execução.
- Os quatro checks do `CONTRIBUTING.md` foram executados. A formatação e Clippy
  gerais continuam com problemas anteriores. Testes do workspace param no
  linker padrão Apple LLVM 21 lendo bitcode LLVM 22. Documentação foi gerada,
  com avisos. Clippy dirigido ao pacote não relatou diagnóstico no arquivo de
  enums alterado, mas encontrou outros problemas existentes.

Comandos reproduzíveis, hashes, oráculos e checks:
[bench/results/2026-10-19-enum-phi/](bench/results/2026-10-19-enum-phi/README.md).
O produtor usou LLVM 22.1.8/ld64.lld e SDK macOS 26.5; os binários desta rodada
rodaram no macOS ARM64. Não há alegação de ganho de desempenho ou certificação
universal. A especialização constante continua experimental e desligada por padrão.
