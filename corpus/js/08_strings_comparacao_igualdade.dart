// Comparação e igualdade de strings: ==, compareTo, ordenação por code units, identical em const e fromCharCodes.
void main() {
  print('abc' == 'abc');
  print('abc' == 'ABC');
  print('abc' != 'abd');
  print('' == '');
  print('a' + 'b' == 'ab');
  var x = 'ab';
  var y = 'a' 'b';
  print(x == y);
  print(identical('lit', 'lit'));
  const c1 = 'const';
  const c2 = 'const';
  print(identical(c1, c2));
  print(identical('a' 'b', 'ab'));
  print(String.fromCharCodes([97, 98]) == 'ab');
  print(String.fromCharCodes([97, 98, 99], 1));
  print(String.fromCharCodes([97, 98, 99], 0, 2));
  print(String.fromCharCodes('xyz'.runes));
  print(String.fromCharCodes([]) == '');

  print('a'.compareTo('b'));
  print('b'.compareTo('a'));
  print('a'.compareTo('a'));
  print('Z'.compareTo('a'));
  print('a'.compareTo('A'));
  print('abc'.compareTo('ab'));
  print('ab'.compareTo('abc'));
  print(''.compareTo('a'));
  print('é'.compareTo('z'));
  print('10'.compareTo('9'));
  print('😀'.compareTo('ｚ'));
  print('\uFFFF'.compareTo('😀'));

  var lista = ['banana', 'Abacaxi', 'abacate', 'Zebra', 'éclair', '10', '9', ''];
  lista.sort();
  print(lista);
  var porLen = ['ccc', 'a', 'bb', 'dd'];
  porLen.sort((a, b) => a.length.compareTo(b.length) != 0
      ? a.length.compareTo(b.length)
      : a.compareTo(b));
  print(porLen);
  var lower = ['banana', 'Abacaxi', 'abacate'];
  lower.sort((a, b) => a.toLowerCase().compareTo(b.toLowerCase()));
  print(lower);
  var inv = ['a', 'c', 'b'];
  inv.sort((a, b) => b.compareTo(a));
  print(inv);

  var s = 'abc';
  print(s == 'abc' && s.length == 3);
  print('abc'.hashCode == 'abc'.hashCode);
  print(('a' + 'bc').hashCode == 'abc'.hashCode);
  Object o = 'abc';
  print(o == 'abc');
  print(o is String);
  print(['a', 'b'].contains('b'));
  print({'k': 1}.containsKey('k'));
  print(['b', 'a', 'c'].reduce((a, b) => a.compareTo(b) <= 0 ? a : b));
  print(['b', 'a', 'c'].reduce((a, b) => a.compareTo(b) >= 0 ? a : b));
  print('a' == 'a' ? 'iguais' : 'diferentes');
  print(x.compareTo(y) == 0);
  var partes = ['ab', 'c'];
  print(partes.join() == 'abc');
  print(partes.join() == s);
  print({'abc', 'a' 'bc', partes.join()}.length);
  print(['b', 'A', 'a', 'B'].toSet().toList()..sort());
  print('a'.codeUnitAt(0) < 'b'.codeUnitAt(0));
  print('abc'.compareTo('abc ') < 0);
  print(' abc'.compareTo('abc') < 0);
  print('ABC'.toLowerCase() == 'abc');
  print('abc' == 'abc'.toString());
  print('Abc'.compareTo('abc').sign);
  print('abc'.compareTo('Abc').sign);
  print('123'.compareTo('45').sign);
  print(int.parse('123').compareTo(int.parse('45')).sign);
}
