import 'package:ngdart/angular.dart';

/// Atributo com `{{ }}`: ligação de propriedade com o valor interpolado
/// (`interpolateString1`, `interpolate2`…), `class` por `updateChildClass`;
/// a expressão primitiva única é conferida crua e interpolada na ação; a
/// constante sai no `if (firstCheck)`.
@Component(
  selector: 'c16-atributo-interpolado',
  templateUrl: 'c16_atributo_interpolado.html',
)
class C16AtributoInterpolado {
  String nome = 'n';
  String cls = 'c';
  int n = 1;
  String a = 'a';
  final String fixo = 'f';
}
