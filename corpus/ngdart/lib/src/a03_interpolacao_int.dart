import 'package:ngdart/angular.dart';

/// Interpolação de um tipo que não é `String`: o oficial usa `interpolate0`
/// em vez de `interpolateString0`.
@Component(
  selector: 'a03-interpolacao-int',
  templateUrl: 'a03_interpolacao_int.html',
)
class A03InterpolacaoInt {
  int quantidade = 3;
}
