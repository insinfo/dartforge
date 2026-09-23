import 'package:ngdart/angular.dart';

import 'd10_filho_var.dart';
import 'd10_filtro.dart';

/// Campos sem tipo escrito: `var x = Classe()` e `var s = 'texto'` têm o tipo
/// inferido; `final p = Enum.a` fica sem tipo, e só pode ir onde o tipo não
/// importa (entrada de filho).
@Component(
  selector: 'd10-campo-var',
  templateUrl: 'd10_campo_var.html',
  directives: [D10FilhoVar],
)
class D10CampoVar {
  var filtro = D10Filtro();
  var nome = 'x';
  final perfil = D10Perfil.a;
}
