import 'package:ngdart/angular.dart';

import 'j58_filho_rotulo.dart';

/// Atributo interpolado num componente filho (o `triggerLabel="{{ x }}"` do
/// `li-dropdown` no limitless_ui): no `@Input`, a interpolação vai para a
/// entrada; fora dele, é propriedade do elemento.
@Component(
  selector: 'j58-atributo-interpolado-no-filho',
  templateUrl: 'j58_atributo_interpolado_no_filho.html',
  directives: [J58FilhoRotulo],
)
class J58AtributoInterpoladoNoFilho {
  String nome = 'n';
  int total = 2;
}
