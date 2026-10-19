# Certificado de clamp e nativos numéricos HIR — 19/10/2026

O certificado de `clamp(0, 1)` exigia `State::Runtime` para as comparações e
conversões numéricas do SDK. Essas mesmas operações já eram `State::Builtin`,
implementadas por `sdk_source::embedded_numeric_native`. Assim, a otimização
experimental recusava o SDK válido antes de gerar qualquer caminho rápido.

A exigência passa a refletir a implementação embutida revisada. Não foi ampliada
para aceitar qualquer estado: bindings desconhecidos, pendentes ou redirecionados
para o runtime continuam recusados. Permanecem as provas de fontes selecionadas,
propriedade das declarações, assinaturas, classes físicas, ausência de aliases e
a auditoria do módulo depois da poda. A opção segue desligada por padrão.

## Verificação

- A falha foi reproduzida antes da edição. Depois, os seis testes dirigidos
  passaram, incluindo alterações no SDK, aliases nativos/FFI, tipos/SSA inválidos,
  preservação de checks de exceção e identidade dos objetos retornados.
- Um produtor congelado compilou OFF/ON a fixture de clamp (373 linhas) e a
  conversão de cores comum da dart_ui (2.736 linhas). Todos os resultados foram
  idênticos aos da VM e do AOT oficiais do Dart 3.6.2, também sob GC stress.
  Incluem NaNs/payloads, zero negativo, inteiros extremos, identidade, avaliação
  única, receptores personalizados, limites inválidos e exceções.
- O LLVM retido contém 0/0/0 guardas de NaN com a opção desligada e 1/3/0 com
  a opção ligada em `unitClamp`, `typedUnitClamp` e `genericClamp`. O caminho
  genérico não ganhou uma especialização sem prova. Não é alegação de desempenho.
- Suíte nativa: **764 passaram, 3 falharam, 9 ignorados**. A falha resolvida é
  exatamente a de clamp; as três restantes são as de preparação ARC do notifier,
  certificação de enum e especialização do kernel de cor, já presentes antes.
- A primeira rodada ampla foi interrompida porque não recebeu o ambiente LLVM
  do build. A rodada configurada completa é a usada nos números acima; um teste
  de contratos que falhara sem essa configuração passou isoladamente com ela.
- Os quatro checks do `CONTRIBUTING.md` foram executados. `fmt` e Clippy ainda
  falham fora desta alteração. O teste do workspace para no linker padrão Apple
  lendo bitcode LLVM 22 com LLVM 21. A documentação foi gerada, com avisos.
  O produtor nativo foi montado com `ld64.lld` 22.1.8 e SDK macOS 26.5.

Reprodução: [scripts/validate-primitive-clamp.py](scripts/validate-primitive-clamp.py).
Comandos, hashes e saídas de referência:
[bench/results/2026-10-19-clamp-embedded/](bench/results/2026-10-19-clamp-embedded/README.md).
A execução de binários desta rodada foi em macOS ARM64; não se atribui a esta
correção uma nova certificação completa de Windows/Linux ou de todo o SDK.
