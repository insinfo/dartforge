import 'package:ngdart/angular.dart';

/// Sonda: índice em `*ngFor` e interpolação.
@Component(
  selector: 'i31-ng-for-campo-indice',
  templateUrl: 'i31_ng_for_campo_indice.html',
  directives: [coreDirectives],
)
class I31NgForCampoIndice {
  List<List<int>> matriz = [[1]];
  Map<String, String> mapa = {'a': 'b'};
}
