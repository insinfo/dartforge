import 'package:ngdart/angular.dart';

/// Interpolação dentro de um atributo: `class="a {{b}}"`.
@Component(
  selector: 'c07-atributo-interpolado',
  templateUrl: 'c07_atributo_interpolado.html',
)
class C07AtributoInterpolado {
  String extra = 'x';
}
