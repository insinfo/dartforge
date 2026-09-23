import 'package:ngdart/angular.dart';

import 'h01_cabecalho.dart';
import 'h01_item.dart';

/// Usa o h01 pela segunda alternativa do seletor, com diretivas numa tag
/// que não é HTML (criadas e descartadas: nenhuma projeção as recebe) e
/// numa `div` projetada; as consultas do filho recebem as instâncias.
@Component(
  selector: 'h01-usa-cabecalho',
  templateUrl: 'h01_usa_cabecalho.html',
  directives: [H01Cabecalho, H01ItemDirective, H01AcoesDirective],
)
class H01UsaCabecalho {}
