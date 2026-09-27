import 'dart:html';

import 'package:ngdart/angular.dart';

/// Sonda: `@ViewChild` com o resultado dois `*ngIf` abaixo.
@Component(
  selector: 'i97-view-child-dois-niveis',
  templateUrl: 'i97_view_child_dois_niveis.html',
  directives: [coreDirectives],
)
class I97ViewChildDoisNiveis {
  bool a = true;
  bool b = true;
  @ViewChild('fundo')
  Element? fundo;
}
