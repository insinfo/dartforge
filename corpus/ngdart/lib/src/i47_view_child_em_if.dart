import 'dart:html';

import 'package:ngdart/angular.dart';

/// Sonda: `@ViewChild` de `#ref` dentro de `*ngIf`.
@Component(
  selector: 'i47-view-child-em-if',
  templateUrl: 'i47_view_child_em_if.html',
  directives: [coreDirectives],
)
class I47ViewChildEmIf {
  bool mostrar = true;
  @ViewChild('caixa')
  Element? caixa;
}
