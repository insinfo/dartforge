import 'package:ngdart/angular.dart';

/// Sonda: `[style.x]` com valor que não é String, unidade com String, nulo e literal.
@Component(
  selector: 'i38-estilo-tipos',
  templateUrl: 'i38_estilo_tipos.html',
  directives: [coreDirectives],
)
class I38EstiloTipos {
  double opacidade = 0.5;
  String tamanho = '2';
  int? topo;
}
