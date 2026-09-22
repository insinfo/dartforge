// Métodos de String: split, trim, pad, replace, substring, indexOf, contains, case, compareTo, *, isEmpty, splitMapJoin.
void main() {
  var s = 'a,b,,c';
  print(s.split(','));
  print(s.split(',').length);
  print('abc'.split(''));
  print('a b  c'.split(' '));
  print('  x y  '.trim());
  print('  x y  '.trimLeft() + '|');
  print('|' + '  x y  '.trimRight());
  print('\t\n x \n'.trim());
  print('5'.padLeft(3, '0'));
  print('5'.padRight(3, '*'));
  print('5'.padLeft(3));
  print('12345'.padLeft(3, '0'));
  print(''.padLeft(2, 'ab'));
  print('banana'.replaceAll('a', 'o'));
  print('banana'.replaceFirst('a', 'o'));
  print('banana'.replaceFirst('a', 'o', 2));
  print('banana'.replaceRange(1, 3, 'XY'));
  print('banana'.replaceAll('', '-'));
  print('hello world'.substring(6));
  print('hello world'.substring(0, 5));
  print('hello'.substring(2, 2).isEmpty);
  print('banana'.indexOf('an'));
  print('banana'.indexOf('an', 2));
  print('banana'.lastIndexOf('an'));
  print('banana'.indexOf('z'));
  print('banana'.lastIndexOf('a', 3));
  print('banana'.contains('nan'));
  print('banana'.contains('nan', 3));
  print('banana'.startsWith('ban'));
  print('banana'.startsWith('ana', 1));
  print('banana'.endsWith('ana'));
  print('Olá Mundo'.toUpperCase());
  print('ÀÉÎÕÜ Ç'.toLowerCase());
  print('ção'.toUpperCase());
  print('abc'.compareTo('abd'));
  print('abc'.compareTo('abc'));
  print('b'.compareTo('a'));
  print('A'.compareTo('a'));
  print('ab' * 3);
  print('x' * 0);
  print(''.isEmpty);
  print('a'.isNotEmpty);
  print('héllo'.runes.map((r) => String.fromCharCode(r)).toList());
  print('a1b22c333'.splitMapJoin(RegExp(r'\d+'),
      onMatch: (m) => '[${m[0]}]', onNonMatch: (n) => n.toUpperCase()));
  print('a-b-c'.splitMapJoin('-', onMatch: (m) => '+'));
  print('a-b-c'.splitMapJoin('-', onNonMatch: (n) => n * 2));
  print('abc'.characters());
  print('hello'.codeUnits.length);
  print('Hello'[0]);
  print('Hello'[4]);
  print('abc'.toUpperCase().toLowerCase() == 'abc');
  print('ab'.compareTo('abc'));
  print('ab'.indexOf(''));
  print('ab'.indexOf('', 2));
  print('ab'.lastIndexOf(''));
  print('a,b'.split(',').map((e) => e.trim()).join('|'));
  print('x'.padRight(0));
  print('abc'.replaceAll('b', ''));
  print('abc'.replaceAll(RegExp('[ac]'), '_'));
  print('aaa'.replaceFirst('a', 'b'));
  print('Mixed Case'.toLowerCase());
  print('mixed case'.toUpperCase());
  print('  '.trim().isEmpty);
  print('a\u00A0b'.trim());
  print('abc'.substring(1));
  print('abcdef'.substring(2, 4));
}

extension on String {
  List<String> characters() => runes.map((r) => String.fromCharCode(r)).toList();
}
