import 'package:ngdart/angular.dart';

import 'j51_rotas.dart';

@Component(
  selector: 'j51-exports',
  templateUrl: 'j51_exports.html',
  exports: [J51Rotas, j51Versao, j51Formatar, J51Modo],
)
class J51Exports {
  J51Modo modo = J51Modo.claro;
}
