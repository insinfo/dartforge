import 'package:ngdart/angular.dart';

/// Chamadas com argumento nomeado no template: o emissor põe vírgula
/// depois de cada um e o `DartFormatter` do builder quebra a lista.
@Component(
  selector: 'j21-argumentos-nomeados',
  templateUrl: 'j21_argumentos_nomeados.html',
)
class J21ArgumentosNomeados {
  void fechar({String origem = 'x'}) {}

  void mover(int a, {bool b = false}) {}

  void aninhar({Object? v}) {}

  String rotulo({bool curto = false}) => curto ? 'r' : 'rotulo';
}
