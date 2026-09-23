import 'package:ngdart/angular.dart';

import 'a11_projecao.dart';

/// Evento num nó projetado num filho: o nó é criado solto e o ouvinte sai
/// com os demais, no fim do `build()` de quem projeta.
@Component(
  selector: 'c18-evento-no-projetado',
  templateUrl: 'c18_evento_no_projetado.html',
  directives: [A11Projecao],
)
class C18EventoNoProjetado {
  int n = 0;
  void clicou() {}
}
