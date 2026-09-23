import 'package:ngdart/angular.dart';

/// Entradas constantes de `*` (`NgIf` direto, `NgFor` pelo `_bindLiteral`
/// com o `if (x != null)`) e ligação constante dentro de visão embutida
/// (`bool firstCheck` declarado lá também).
@Component(
  selector: 'a26-constantes-em-embutida',
  templateUrl: 'a26_constantes_em_embutida.html',
  directives: [coreDirectives],
)
class A26ConstantesEmEmbutida {
  final bool fixo = true;
  final List<String> itens = const ['a', 'b'];
}
