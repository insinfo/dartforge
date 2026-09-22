import 'package:ngdart/angular.dart';

/// `String?` e `final int`: as duas pontas da regra de escolha entre
/// `interpolateString`, `interpolate` e `updateTextWithPrimitive`.
@Component(
  selector: 'a18-interpolacao-nula',
  templateUrl: 'a18_interpolacao_nula.html',
)
class A18InterpolacaoNula {
  String? talvez;
  final int fixo = 1;
}
