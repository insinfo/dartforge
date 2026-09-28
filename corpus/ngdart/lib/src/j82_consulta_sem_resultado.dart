import 'dart:html';

import 'package:ngdart/angular.dart';

/// `@ViewChild`/`@ViewChildren` cujo `#ref` não existe no template (o
/// `treeContainer` do `li-treeview`).
@Component(
  selector: 'j82-consulta-sem-resultado',
  template: '<div #outro></div>',
)
class J82ConsultaSemResultado {
  @ViewChild('ausente')
  DivElement? ausente;

  @ViewChildren('ausentes')
  List<Element> ausentes = [];

  @ViewChild('outro')
  DivElement? outro;
}
