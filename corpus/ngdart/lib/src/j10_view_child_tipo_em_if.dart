import 'package:ngdart/angular.dart';

import 'a02_texto_estatico.dart';

/// Sonda: `@ViewChild(Tipo)` de componente filho dentro de `*ngIf`.
@Component(
  selector: 'j10-view-child-tipo-em-if',
  templateUrl: 'j10_view_child_tipo_em_if.html',
  directives: [coreDirectives, A02TextoEstatico],
)
class J10ViewChildTipoEmIf {
  bool a = true;
  @ViewChild(A02TextoEstatico)
  A02TextoEstatico? filho;
}
