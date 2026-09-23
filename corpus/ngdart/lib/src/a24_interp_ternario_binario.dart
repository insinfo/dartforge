import 'package:ngdart/angular.dart';

/// Ternário, `!x`, `??` e binário são `dynamic` (`interpolate0`), menos
/// `String + String` (`interpolateString0`); literal vira o próprio texto,
/// escrito uma vez no `build()`.
@Component(
  selector: 'a24-interp-ternario-binario',
  templateUrl: 'a24_interp_ternario_binario.html',
)
class A24InterpTernarioBinario {
  bool ativo = true;
  String a = 'a';
  String b = 'b';
  int n = 1;
  String? talvez;
}
