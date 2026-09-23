# Bibliotecas e imports relativos

O compilador liga ASTs de cada biblioteca, sem concatenar nem reescrever os arquivos Dart. `dartforge-packages` carrega o grafo e `dartforge-linker::compile_graph` analisa, resolve, valida e emite um único módulo JavaScript.

## Suportado

- `import 'arquivo.dart';` e `export 'arquivo.dart';` relativos, com combinadores `show`/`hide`, inclusive diretórios, ciclos declarativos e diamantes. Caminhos canônicos identificam arquivos; imports repetidos são idempotentes.
- Cada arquivo é uma biblioteca. Somente declarações próprias e declarações públicas de imports **diretos** são visíveis. Imports não são transitivos e não equivalem a `export`.
- Funções e classes recebem nomes distintos por biblioteca na AST. Classes recebem IDs globais antes do parsing dos corpos. Nomes iguais em bibliotecas separadas não colidem na emissão.
- Membros e declarações iniciados por `_` são privados da biblioteca. A renomeação privada inclui a biblioteca declarante; membros privados de uma base estrangeira não sobrescrevem privados homônimos da derivada.
- Declarações próprias prevalecem sobre imports. Sombreamento local, parâmetros e declarações posteriores são verificados antes da renomeação, inclusive nomes de tipos e construtores.
- Herança e chamadas entre arquivos seguem a mesma verificação nominal do compilador de uma unidade.
- A unidade de entrada deve declarar `void main()` sem parâmetros. Uma biblioteca importada pode declarar seu próprio `main`, que recebe um nome independente e não é executado automaticamente.
- Diagnósticos mantêm o caminho e os offsets em bytes do arquivo original. A análise conjunta usa um domínio temporário de offsets, posteriormente convertido para o arquivo correspondente.
- A otimização opcional de constantes só executa após a validação completa do programa ligado.

## Limites explícitos

- Não há `deferred`, que tem diagnóstico próprio. `import '...' as p;` é suportado e documentado em [IMPORTS.md](IMPORTS.md): o namespace importado passa a ser alcançável apenas por `p.nome`, e `p` sozinho é erro. `export '...' as p;` não existe na gramática de Dart e é recusado. `part`, `part of` e `library` são suportados e documentados em [PARTS.md](PARTS.md): a parte não é uma biblioteca, mas compartilha namespace, imports — inclusive prefixos — e privacidade com o arquivo que a declara. Combinadores `show` e `hide` são suportados em imports e exports, e compõem com o prefixo. A resolução de `package:` segue o suporte de package_config v2 oferecido pelo carregador.
- Ambiguidades entre imports diretos são rejeitadas mesmo quando o nome não é usado. Uma declaração própria com aquele nome resolve a colisão. Use `show`/`hide` ou um prefixo `as` para desambiguar; um nome alcançado por prefixo não concorre com um nome importado sem prefixo, porque ele não entra no namespace sem qualificação.
- Extensions ainda são rejeitadas pela compilação do grafo, para não atribuir alcance ou precedência incorretos a extensions importadas.
- Chamadas de membros exigem receptor explícito; referências implícitas não são vinculadas silenciosamente a funções globais homônimas.
- O namespace gerado usa `$lib<ID>$nome`, não aceito pelo lexer atual como identificador de usuário. Se o lexer passar a aceitar `$`, essa reserva precisa ser revista antes da ampliação.
- Não há isolamento físico entre módulos JavaScript nem source maps. A privacidade é verificada pelo compilador DartForge; não é uma fronteira de segurança contra JavaScript externo.
- Exports conflitantes são erros; reexportar a mesma origem por vários caminhos é idempotente. Nomes próprios da biblioteca prevalecem sobre exports e imports.
- Todos os arquivos alcançados são analisados e emitidos; não há tree shaking, compilação incremental de módulos ou carregamento tardio.
- `SourceGraph` passado diretamente à API deve obedecer ao contrato de `dartforge-packages::load`, incluindo spans de diretivas e arquivos UTF-8 originais.

## Validação

Testes cobrem imports diretos, privacidade de funções/campos, diamantes e ciclos, main de biblioteca, classes entre unidades, sombreamento anterior à declaração, ambiguidade e caminhos de erros. Um teste com Node executa ambos os modos, direto e otimizado, incluindo privados homônimos em uma cadeia de herança.
