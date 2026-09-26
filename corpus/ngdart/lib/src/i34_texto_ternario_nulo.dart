import 'package:ngdart/angular.dart';

/// Sonda: `??`, ternário, `.isEmpty`.
@Component(
  selector: 'i34-texto-ternario-nulo',
  templateUrl: 'i34_texto_ternario_nulo.html',
  directives: [coreDirectives],
)
class I34TextoTernarioNulo {
  String? nome;
  bool ativo = true;
  List<int> lista = [];
}
