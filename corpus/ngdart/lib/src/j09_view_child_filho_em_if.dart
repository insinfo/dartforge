import 'package:ngdart/angular.dart';

import 'a02_texto_estatico.dart';

/// Sonda: `@ViewChild('ref')` de componente filho dentro de `*ngIf`.
@Component(
  selector: 'j09-view-child-filho-em-if',
  templateUrl: 'j09_view_child_filho_em_if.html',
  directives: [coreDirectives, A02TextoEstatico],
)
class J09ViewChildFilhoEmIf {
  bool a = true;
  @ViewChild('f')
  A02TextoEstatico? filho;
}
