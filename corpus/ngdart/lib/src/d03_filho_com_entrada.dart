import 'package:ngdart/angular.dart';

import 'a16_entrada_e_saida.dart';
import 'a16_entrada_e_saida.template.dart';

/// Componente filho com `@Input` ligado.
@Component(
  selector: 'd03-filho-com-entrada',
  templateUrl: 'd03_filho_com_entrada.html',
  directives: [A16EntradaESaida],
)
class D03FilhoComEntrada {
  String valor = 'x';
}
