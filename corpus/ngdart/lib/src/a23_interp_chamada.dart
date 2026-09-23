import 'package:ngdart/angular.dart';

class Conta {
  String descricao() => 'x';
  int total() => 1;
}

/// Interpolação de chamada, índice e `x!`, pelo `_TypeResolver` do oficial:
/// a chamada tem o tipo do retorno (`String` → `interpolateString0`, `int`
/// → `updateTextWithPrimitive`), o índice e `x!` são `dynamic`
/// (`interpolate0`, com `(x!)` entre parênteses).
@Component(
  selector: 'a23-interp-chamada',
  templateUrl: 'a23_interp_chamada.html',
)
class A23InterpChamada {
  Conta conta = Conta();
  List<String> nomes = ['a'];
  String? talvez = 'x';
  String titulo() => 't';
  int contar() => 2;
}
