import 'package:ngdart/angular.dart';
import 'package:ngforms/ngforms.dart';

import 'j30_caixa.dart';

/// `NgModel` no elemento de um componente que provê o acessor de valor, e
/// o mesmo componente sem `NgModel`: ali o `NgValueAccessor` do nó é
/// preguiçoso (campo `late` com inicializador).
@Component(
  selector: 'j31-usa-caixa',
  templateUrl: 'j31_usa_caixa.html',
  directives: [J30Caixa, formDirectives],
)
class J31UsaCaixa {
  String valor = '';
}