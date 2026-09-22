// Erros de int.parse/double.parse: FormatException, tryParse null, espaços, '1.0', vírgula, radix inválido e dígitos fora do radix.
bool falhaInt(String s, {int? radix}) {
  try {
    int.parse(s, radix: radix);
    return false;
  } on FormatException {
    return true;
  }
}

bool falhaDouble(String s) {
  try {
    double.parse(s);
    return false;
  } on FormatException {
    return true;
  }
}

void main() {
  try {
    int.parse('abc');
  } catch (e) {
    print(e is FormatException);
  }
  try {
    int.parse('abc');
  } on FormatException catch (e) {
    print(e.message.isNotEmpty || e.message.isEmpty);
  }
  try {
    int.parse('');
  } on FormatException catch (e) {
    print('vazio: ${e is FormatException}');
  }
  print(falhaInt('1.0'));
  print(falhaInt('1.5'));
  print(falhaInt('1e3'));
  print(falhaInt('1 2'));
  print(falhaInt('1_000'));
  print(falhaInt('1,000'));
  print(falhaInt('0x'));
  print(falhaInt('x10'));
  print(falhaInt('+-1'));
  print(falhaInt('++1'));
  print(falhaInt('1+'));
  print(falhaInt('١'));
  print(falhaInt('１２'));
  print(falhaInt('Infinity'));
  print(falhaInt('NaN'));
  print(falhaInt('true'));
  print(falhaInt('null'));
  print(falhaInt(' '));
  print(falhaInt('-'));
  print(falhaInt('+'));
  print(falhaInt(' 12 '));
  print(falhaInt('\t12\n'));
  print(falhaInt('12', radix: 2));
  print(falhaInt('102', radix: 2));
  print(falhaInt('8', radix: 8));
  print(falhaInt('g', radix: 16));
  print(falhaInt('F', radix: 16));
  print(falhaInt('0x10', radix: 16));
  print(falhaInt('0x10', radix: 10));
  print(falhaInt('z', radix: 36));
  print(falhaInt('Z', radix: 36));
  print(falhaInt('a', radix: 10));
  print(falhaInt('9', radix: 9));
  print(falhaInt('10', radix: 3));
  try {
    int.parse('10', radix: 1);
    print('sem erro');
  } on RangeError {
    print('RangeError');
  } on ArgumentError {
    print('ArgumentError');
  }
  try {
    int.parse('10', radix: 37);
    print('sem erro');
  } on RangeError {
    print('RangeError');
  } on ArgumentError {
    print('ArgumentError');
  }
  try {
    int.parse('10', radix: 0);
    print('sem erro');
  } on RangeError {
    print('RangeError');
  } on ArgumentError {
    print('ArgumentError');
  }
  print(int.tryParse('abc'));
  print(int.tryParse('1.0'));
  print(int.tryParse('12 3'));
  print(int.tryParse('12', radix: 2));
  print(int.tryParse('11', radix: 2));
  print(int.tryParse('0x10', radix: 16));
  print(int.tryParse('10', radix: 16));
  print(int.tryParse(' 42\n'));
  print(int.tryParse('4 2'));
  print(int.tryParse('٤٢'));
  print(int.tryParse('4２'));

  print(falhaDouble('abc'));
  print(falhaDouble(''));
  print(falhaDouble('1,5'));
  print(falhaDouble('1.5.5'));
  print(falhaDouble('1..5'));
  print(falhaDouble('.'));
  print(falhaDouble('1e'));
  print(falhaDouble('e1'));
  print(falhaDouble('1e1.5'));
  print(falhaDouble('0x10'));
  print(falhaDouble('1 .5'));
  print(falhaDouble('1. 5'));
  print(falhaDouble('--1.5'));
  print(falhaDouble('+-1.5'));
  print(falhaDouble('1.5-'));
  print(falhaDouble('nan'));
  print(falhaDouble('NAN'));
  print(falhaDouble('inf'));
  print(falhaDouble('infinity'));
  print(falhaDouble('Infinity'));
  print(falhaDouble('-Infinity'));
  print(falhaDouble('NaN'));
  print(falhaDouble('1_000.5'));
  print(falhaDouble('１.５'));
  print(falhaDouble(' 1.5 '));
  print(falhaDouble('1.5\n'));
  print(falhaDouble('1.'));
  print(falhaDouble('.5'));
  print(falhaDouble('-.5'));
  print(falhaDouble('1e+'));
  print(falhaDouble('1e-'));
  print(falhaDouble('1E5'));
  print(falhaDouble('1e05'));
  print(falhaDouble('true'));
  print(double.tryParse('1,5'));
  print(double.tryParse('x'));
  print(double.tryParse('1.5x'));
  print(double.tryParse('1.25'));
  print(double.tryParse('   -0.5   '));
  print(num.tryParse('abc'));
  print(num.tryParse('1.5.5'));
  print(num.tryParse('0x'));
  print(num.tryParse('1.5'));
  print(num.tryParse('15'));
  try {
    num.parse('nada');
  } catch (e) {
    print(e is FormatException);
  }
  var entradas = ['1', '2.5', 'x', '', ' 3 ', '4x', '-5', '+6', '0x7', '1e1x'];
  print(entradas.map((e) => int.tryParse(e) ?? 'E').toList());
  print(entradas.map((e) => double.tryParse(e) == null ? 'E' : 'ok').toList());
  print(entradas.where((e) => int.tryParse(e) != null).toList());
  print(entradas.where((e) => double.tryParse(e) != null).toList());
  print(entradas.map((e) => falhaInt(e) ? 1 : 0).toList());
  print(entradas.map((e) => falhaDouble(e) ? 1 : 0).toList());
}
