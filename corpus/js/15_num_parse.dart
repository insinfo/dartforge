// int.parse/tryParse, double.parse/tryParse, num.parse: decimal, sinal, radix, 0x, NaN/Infinity e falhas.
void main() {
  print(int.parse('42'));
  print(int.parse('-42'));
  print(int.parse('+42'));
  print(int.parse('0'));
  print(int.parse('007'));
  print(int.parse(' 42 '));
  print(int.parse('1010', radix: 2));
  print(int.parse('ff', radix: 16));
  print(int.parse('FF', radix: 16));
  print(int.parse('-ff', radix: 16));
  print(int.parse('0xff'));
  print(int.parse('0XFF'));
  print(int.parse('-0x10'));
  print(int.parse('777', radix: 8));
  print(int.parse('z', radix: 36));
  print(int.parse('Zz', radix: 36));
  print(int.tryParse('123'));
  print(int.tryParse('abc'));
  print(int.tryParse(''));
  print(int.tryParse('1.5'));
  print(int.tryParse('1e3'));
  print(int.tryParse('12', radix: 2));
  print(int.tryParse('0x1F'));
  print(int.tryParse('0x1F', radix: 16));
  print(int.tryParse(' 7'));
  print(int.tryParse('7 '));
  print(int.tryParse('- 7'));
  print(int.tryParse('--7'));
  print(double.parse('3.5'));
  print(double.parse('-3.5'));
  print(double.parse('+0.25'));
  print(double.parse('.5'));
  print(double.parse('2.55e1'));
  print(double.parse('1.5E-3'));
  print(double.parse('2.5e-1'));
  print(double.parse('NaN').isNaN);
  print(double.parse('Infinity'));
  print(double.parse('-Infinity'));
  print(double.parse(' 2.5 '));
  print(double.parse('0.1') + double.parse('0.2'));
  print(double.parse('1e-7'));
  print(double.parse('1.5').toStringAsFixed(3));
  print(double.parse('10').toStringAsFixed(1));
  print(double.tryParse('x'));
  print(double.tryParse(''));
  print(double.tryParse('1,5'));
  print(double.tryParse('2.5'));
  print(double.tryParse('nan'));
  print(double.tryParse('infinity'));
  print(double.tryParse('1.5.5'));
  print(double.tryParse('0x10')?.toStringAsFixed(1));
  print(num.parse('42'));
  print(num.parse('4.5'));
  print(num.parse('-4.5'));
  print(num.parse('0x1A'));
  print(num.parse('42') is int);
  print(num.parse('4.5') is double);
  print(num.parse('1e2').toStringAsFixed(1));
  print(num.tryParse('nada'));
  print(num.tryParse('7'));
  print(num.tryParse('7.5'));
  try {
    int.parse('abc');
  } catch (e) {
    print('lançou ${e is FormatException}');
  }
  try {
    double.parse('abc');
  } catch (e) {
    print('lançou ${e is FormatException}');
  }
  try {
    int.parse('10', radix: 1);
  } catch (e) {
    print('radix inválido ${e is RangeError || e is ArgumentError}');
  }
  print(int.parse('9007199254740991'));
  print(int.parse('-9007199254740992'));
  print(int.parse('   -0x1f   '));
  print(int.tryParse('0b101'));
  print(int.parse('101', radix: 2) + int.parse('11', radix: 2));
  print(int.parse('7') + double.parse('0.5'));
  print(['1', '2', 'x', '4'].map(int.tryParse).toList());
  print(['1.5', 'x'].map(double.tryParse).toList());
  print(int.parse('+0'));
  print(int.parse('00'));
  print(int.tryParse('+'));
  print(int.tryParse('-'));
  print(int.tryParse('0x'));
  print(double.tryParse('.'));
  print(double.tryParse('-.5'));
  print(double.tryParse('5.')?.toStringAsFixed(1));
  print(double.tryParse('1e'));
  print(double.tryParse('e5'));
  print(double.tryParse('+NaN')?.isNaN);
  print(double.tryParse('-NaN')?.isNaN);
  print(double.tryParse('+Infinity'));
  print(int.parse('123', radix: 10));
  print(int.parse('123', radix: 4));
  print(int.tryParse('124', radix: 4));
  print(int.tryParse('9', radix: 8));
  print(int.tryParse('8', radix: 8));
  print(int.tryParse('7', radix: 8));
}
