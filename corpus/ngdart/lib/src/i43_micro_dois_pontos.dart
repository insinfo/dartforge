import 'package:ngdart/angular.dart';

/// Sonda: `:` dentro da expressão da microssintaxe.
@Component(
  selector: 'i43-micro-dois-pontos',
  templateUrl: 'i43_micro_dois_pontos.html',
  directives: [coreDirectives],
)
class I43MicroDoisPontos {
  bool ativo = true;
  bool mostrar = true;
  String modo = 'a';
}
