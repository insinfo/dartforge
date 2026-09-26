import 'dart:html';

import 'package:ngdart/angular.dart';

import 'a02_texto_estatico.dart';

/// Sonda: `@ViewChildren` de filhos, `@ViewChild` de `#ref` repetido e consulta sem resultado.
@Component(
  selector: 'i49-view-children-formas',
  templateUrl: 'i49_view_children_formas.html',
  directives: [A02TextoEstatico],
)
class I49ViewChildrenFormas {
  @ViewChildren('f')
  List<A02TextoEstatico>? filhos;
  @ViewChild('p')
  Element? primeiro;
  @ViewChildren('nada')
  List<Element>? nenhum;
}
