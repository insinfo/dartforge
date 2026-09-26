import 'package:ngdart/angular.dart';

import 'a02_texto_estatico.dart';

/// Sonda: `#ref` de componente filho em evento.
@Component(
  selector: 'i32-evento-filho-ref',
  templateUrl: 'i32_evento_filho_ref.html',
  directives: [coreDirectives, A02TextoEstatico],
)
class I32EventoFilhoRef {
  void usar(A02TextoEstatico t) {}
}
