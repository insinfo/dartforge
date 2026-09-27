import 'dart:html';

import 'package:ngdart/angular.dart';

/// Sonda: `@ViewChildren` com vários resultados dentro de `*ngFor`.
@Component(
  selector: 'i96-view-children-em-for',
  templateUrl: 'i96_view_children_em_for.html',
  directives: [coreDirectives],
)
class I96ViewChildrenEmFor {
  List<String> itens = ['a', 'b'];
  @ViewChildren('item')
  List<Element>? elementos;
}
