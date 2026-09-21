# Partes de biblioteca (`part` e `part of`)

Uma biblioteca pode distribuir suas declarações por vários arquivos. O arquivo que
declara `part 'x.dart';` continua sendo a biblioteca; `x.dart` é uma **parte** e não
possui escopo próprio. `dartforge-packages` resolve as diretivas e `dartforge-linker`
junta as declarações de pai e partes em um único namespace, conforme
[a documentação de bibliotecas](MODULES.md).

## Suportado

- `part 'relativo.dart';` e `part 'package:p/arquivo.dart';` depois de `library` e de
  todos os `import`/`export`, na ordem exigida pela gramática do Dart 3.6.2.
- `part of 'pai.dart';` (URI relativa resolvida para o arquivo pai) e
  `part of nome.da.biblioteca;`, aceito quando o pai declara `library nome.da.biblioteca;`.
  A diretiva `part of` deve ser a primeira do arquivo.
- As declarações de topo da parte pertencem à biblioteca pai: funções, classes, enums e
  mixins entram no mesmo namespace, são renomeadas com o ID da biblioteca (`$lib<ID>$nome`)
  e colidem com homônimos do pai ou de outra parte (`símbolo top-level duplicado`).
- Pai e partes compartilham os mesmos `import`/`export`, inclusive prefixos e combinadores
  `show`/`hide`, e o mesmo `dart:core`/`dart:async`/`dart:ffi` visível.
- Privacidade é por biblioteca: `_nome` declarado na parte é visível no pai e vice-versa,
  inclusive membros privados de classes. A renomeação privada usa o ID da biblioteca, então
  privados homônimos de bibliotecas diferentes continuam distintos.
- `void main()` da entrada pode ser declarado no arquivo da biblioteca ou em qualquer
  parte dela; a chamada gerada usa o nome da biblioteca.
- Diagnósticos apontam o arquivo em que o erro está e os offsets em bytes daquele arquivo,
  mesmo quando o erro é semântico e só aparece depois da ligação.
- A parte é uma unidade do `SourceGraph` (`SourceUnit::part_of`), com fonte e caminho
  canônico próprios. Como a sessão compara o grafo inteiro, editar uma parte invalida o
  cache da biblioteca pai (veja [CACHE.md](CACHE.md)).

## Erros explícitos

- Parte com `import`, `export`, `library` ou `part` próprios.
- Arquivo incluído por `part` sem `part of`, ou com `part of` apontando para outro arquivo
  ou para outro nome de biblioteca.
- `part of` em um arquivo que nenhuma biblioteca declara como parte, inclusive na entrada.
- O mesmo arquivo reivindicado por duas bibliotecas, `part` duplicado na mesma biblioteca,
  biblioteca que declara a si mesma como parte e `part` de um arquivo já carregado como
  biblioteca. Cada caso é diagnosticado na diretiva que provocou o conflito, sem laço:
  uma parte nunca declara outra parte, então não existe cadeia a percorrer.
- `import`/`export` cujo destino já é parte de outra biblioteca.
- `part` fora do prefixo de diretivas, `part of` depois de outra diretiva e URIs com
  query, fragmento, escapes ou esquemas não suportados.

## Limites explícitos

- Partes aprimoradas (parte com `import`/`export` próprios ou `part` aninhado), previstas
  depois do Dart 3.6.2, não são aceitas.
- `part of` por nome exige `library` idêntico, sem normalização de espaços ou maiúsculas;
  nomes de biblioteca não entram em nenhum namespace e servem apenas para essa verificação.
- Uma parte não pode ser entrada de compilação nem ser importada; use a biblioteca.
- O comando `dartforge graph` ainda imprime somente imports e exports de cada unidade.
- Um arquivo que declara `library` sem nenhum `import`, `export` ou `part` cai na rota de
  arquivo único de `dartforge-compiler`, que entrega a fonte inteira ao parser e ainda não
  ignora o prefixo de diretivas; o diagnóstico é o do parser, não o da biblioteca. Com pelo
  menos uma dessas diretivas o arquivo segue pela ligação normal e o `library` é aceito.
- `SourceGraph` construído à mão deve manter `parts` e `part_of` coerentes: a parte precisa
  ser reivindicada por exatamente uma biblioteca, sem imports, exports ou partes próprias.
  O linker rejeita o grafo inconsistente em vez de adivinhar o dono.

## Verificação

    cargo test -p dartforge-packages
    cargo test -p dartforge-linker
    cargo test -p dartforge-compiler --test parts -- --include-ignored

Os testes cobrem resolução por URI e por nome de biblioteca, spans das diretivas,
reivindicação exclusiva do arquivo, parte com `import`, `part of` divergente, namespace e
privacidade compartilhados, `main` declarado na parte e a execução em Node do programa
ligado com uma parte.
