import 'package:ngdart/angular.dart';

import 'a02_texto_estatico.dart';

/// Sonda: `@ViewChild(Tipo)` de componente filho.
@Component(
  selector: 'i16-view-child-componente',
  templateUrl: 'i16_view_child_componente.html',
  directives: [coreDirectives, A02TextoEstatico],
)
class I16ViewChildComponente {
  @ViewChild(A02TextoEstatico)
  A02TextoEstatico? filho;
}
