import 'dart:html';

import 'package:ngdart/angular.dart';

/// Sonda: `@ViewChildren` com os resultados num `*ngFor` dentro de `*ngIf`.
@Component(
  selector: 'j06-view-children-dois-niveis',
  templateUrl: 'j06_view_children_dois_niveis.html',
  directives: [coreDirectives],
)
class J06ViewChildrenDoisNiveis {
  bool a = true;
  List<String> xs = ['a'];
  @ViewChildren('item')
  List<Element>? itens;
}
