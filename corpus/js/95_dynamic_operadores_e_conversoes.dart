// dynamic com operadores (+ - * comparação), interpolação, as, if (d), switch, Object vs dynamic, toString.
Object caixaObject(Object o) => o;
dynamic caixaDynamic(dynamic d) => d;

String descreve(dynamic d) {
  switch (d) {
    case 1:
      return 'um';
    case 'dois':
      return 'texto dois';
    case true:
      return 'verdadeiro';
    case null:
      return 'nulo';
    default:
      return 'outro: $d';
  }
}

void main() {
  dynamic a = 7;
  dynamic b = 3;
  print(a + b);
  print(a - b);
  print(a * b);
  print(a ~/ b);
  print(a % b);
  print(a > b);
  print(a < b);
  print(a >= 7);
  print(a == 7);
  print(a == '7');
  print(a != b);
  print(-a);
  print(a & b);
  print(a | b);
  print(a ^ b);
  print(a << 2);
  print(a >> 1);

  dynamic s1 = 'foo';
  dynamic s2 = 'bar';
  print(s1 + s2);
  print(s1 * 2);
  print(s1 == 'foo');
  print(s1.compareTo(s2));
  print(s1 + a.toString());
  try {
    print(s1 + a);
  } catch (e) {
    print('string + int: ${e is TypeError || e is ArgumentError}');
  }
  try {
    print(a + s1);
  } catch (e) {
    print('int + string: ${e is TypeError || e is ArgumentError}');
  }

  print('a=$a b=$b s=$s1');
  print('${a + b} ${s1 + s2}');
  dynamic lista = [1, 2];
  print('lista=$lista');
  dynamic nulo;
  print('nulo=$nulo');
  print('${nulo}${nulo}');

  final int i = a as int;
  print(i + 1);
  final String str = s1 as String;
  print(str.length);
  final num nn = a as num;
  print(nn);
  try {
    final String errado = a as String;
    print(errado);
  } catch (e) {
    print('as errado: ${e is TypeError}');
  }
  final int? talvez = nulo as int?;
  print(talvez);
  final List<int> li = lista as List<int>;
  print(li.length);

  dynamic flag = true;
  if (flag) {
    print('flag verdadeira');
  }
  if (!flag) print('nunca');
  print(flag && a > 1);
  print(flag ? 'sim' : 'nao');
  dynamic naoBool = 'texto';
  try {
    if (naoBool) print('nunca');
  } catch (e) {
    print('if(string): ${e is TypeError}');
  }
  while (flag) {
    flag = false;
  }
  print(flag);

  print(descreve(1));
  print(descreve('dois'));
  print(descreve(true));
  print(descreve(null));
  print(descreve(2.5));
  print(descreve([1]));

  final Object o = 'objeto';
  print((o as String).length);
  print(o is String ? o.length : -1);
  print(o.toString());
  print(o.hashCode == o.hashCode);
  print(o.runtimeType == String);
  dynamic dd = o;
  print(dd.length);
  print(caixaObject(5) is int);
  print(caixaDynamic(5) + 1);
  final Object oo = caixaDynamic('x');
  print(oo);
  dynamic ddd = caixaObject(4);
  print(ddd * 2);

  dynamic d = 12;
  print(d.toString());
  print(d.toString().length);
  d = 'x';
  print(d.toString());
  d = [1];
  print(d.toString());
  d = null;
  print(d.toString());
  d = {'k': 'v'};
  print(d.toString());
  d = 1.5;
  print(d.toString());
  print(d.toStringAsFixed(2));
}
