// Constante que espalha uma `const` de OUTRA biblioteca
// (docs/NATIVO-PROJETOS-REAIS.md, causa C3). A chave canônica de
// `[0, ...pequena, 3]` lia o inicializador de `pequena` com a AST de
// main.dart: com o índice dentro da arena saía outra expressão (a chave
// errada: `identical` falso); fora dela, pânico (ast.rs:55, o
// new_sali/backend). Especificação §17.3: as duas expressões denotam o
// mesmo objeto.

import 'constantes.dart';
import 'constantes.dart' as c;

const lista = [0, ...pequena, 3];
const listaPrefixada = [0, ...c.pequena, 3];
const mapaMaior = {...mapa, 'c': 3};
const conjuntoMaior = {...conjunto, 'z'};
const comAninhada = [...aninhada, 9];

void main() {
  print(lista);
  print(mapaMaior);
  print(conjuntoMaior);
  print(comAninhada);
  print(identical(lista, const [0, 1, 2, 300000, 4, 5, 3]));
  print(identical(lista, listaPrefixada));
  print(identical(mapaMaior, const {'a': 1, 'bb': 2, 'c': 3}));
  print(identical(conjuntoMaior, const {'x', 'y', 'z'}));
  print(identical(comAninhada[0], pequena) || identical(comAninhada[0], const [1, 2]));
  print(identical(const [...pequena], pequena));
}
