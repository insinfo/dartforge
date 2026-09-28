import 'dart:html';

import 'package:ngdart/angular.dart';

/// Consultas declaradas numa ordem e com resultados na ordem inversa dentro
/// de um `*ngIf` (o `li-datatable`: `#table` dentro de `#scrollContainer`):
/// o `dirtyParentQueriesInternal` segue os resultados, e os campos dos nós
/// seguem as consultas.
@Component(
  selector: 'j66-sujas-em-ordem',
  templateUrl: 'j66_sujas_em_ordem.html',
  directives: [NgIf],
)
class J66SujasEmOrdem {
  bool mostrar = true;
  String classe = 'c';

  @ViewChild('tabela')
  TableElement? tabela;

  @ViewChild('rolagem')
  DivElement? rolagem;
}
