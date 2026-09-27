// Divisão inteira por zero com `int` sem caixa: `~/` e `%` lançam o
// `IntegerDivisionByZeroException` do `dart:core` (antes, o `~/` lançava um
// objeto de erro do runtime antigo e o `%` abortava no native).
int div(int a, int b) => a ~/ b;
int resto(int a, int b) => a % b;

void main() {
  final zero = int.parse('0');
  for (final f in [() => div(7, zero), () => resto(7, zero), () => 7.remainder(zero), () => (-7) ~/ zero]) {
    try {
      print(f());
    } on IntegerDivisionByZeroException catch (e) {
      print('pegou ${e.runtimeType}: $e');
    } catch (e) {
      print('outro ${e.runtimeType}');
    }
  }
  int Function(int) cem = (x) => 100 ~/ x;
  try {
    cem(zero);
  } on UnsupportedError {
    print('nunca');
  } on IntegerDivisionByZeroException {
    print('closure tipada');
  }
  var passou = false;
  try {
    try {
      resto(1, zero);
    } finally {
      passou = true;
    }
  } catch (e) {
    print('finally $passou ${e is IntegerDivisionByZeroException}');
  }
  print('${div(-7, 2)} ${resto(-7, 2)} ${div(-9223372036854775808, -1)} ${resto(-9223372036854775808, -1)}');
}
