import 'package:ngdart/angular.dart';

import 'a02_texto_estatico.dart';
import 'a11_projecao.dart';

/// Sonda: `#ref` no conteúdo projetado lido num evento.
@Component(
  selector: 'i48-ref-projetado-lido',
  templateUrl: 'i48_ref_projetado_lido.html',
  directives: [A11Projecao, A02TextoEstatico],
)
class I48RefProjetadoLido {
  void usar(Object? a, Object? b) {}
}
