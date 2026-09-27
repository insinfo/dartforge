import 'package:ngdart/angular.dart';

import 'i68_marca.dart';
import 'i80_consultas_leitura.dart';
import 'i80_rotulo.dart';

/// Sonda: filho com consultas `read:` recebendo conteúdo; o primeiro nó
/// achado tem ligação de propriedade (é campo da visão).
@Component(
  selector: 'i81-usa-consultas-leitura',
  templateUrl: 'i81_usa_consultas_leitura.html',
  directives: [I80ConsultasLeitura, I68Marca, I80Rotulo],
)
class I81UsaConsultasLeitura {
  String t = 'x';
}
