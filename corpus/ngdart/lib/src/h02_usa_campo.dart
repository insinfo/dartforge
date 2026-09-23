import 'package:ngdart/angular.dart';
import 'package:ngforms/ngforms.dart';

import 'h02_campo.dart';

/// `NgModel` no elemento de um componente que provê o acessor de valor, e
/// o mesmo componente sem `NgModel`: ali o `NgValueAccessor` do nó é
/// preguiçoso (campo `late` com inicializador).
@Component(
  selector: 'h02-usa-campo',
  templateUrl: 'h02_usa_campo.html',
  directives: [H02Campo, formDirectives],
)
class H02UsaCampo {
  String valor = '';
}