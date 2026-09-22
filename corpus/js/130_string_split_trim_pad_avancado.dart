// String avançado: split com vazio e separadores nas pontas, split por regex, trim de whitespace, pad multi-char, replaceAll vazio, splitMapJoin, indexOf com start.
void main() {
  print('abc'.split(''));
  print(''.split(''));
  print(''.split(','));
  print(','.split(','));
  print(',a,'.split(','));
  print('a,,b'.split(','));
  print(',,'.split(','));
  print('a'.split(','));
  print('a,b'.split(',,'));
  print('aXbXc'.split('X'));
  print('a::b::c'.split('::'));
  print('abc'.split('abc'));
  print('abcabc'.split('abc'));
  print('xabcx'.split('abc'));
  print('a1b2c3'.split(RegExp(r'\d')));
  print('1a2b3'.split(RegExp(r'\d')));
  print('a  b   c'.split(RegExp(r' +')));
  print('a  b   c'.split(' '));
  print('a\tb\nc'.split(RegExp(r'\s')));
  print('a,b;c d'.split(RegExp(r'[,; ]')));
  print('camelCaseString'.split(RegExp(r'(?=[A-Z])')));
  print('a1b'.split(RegExp(r'(\d)')));
  print('abc'.split(RegExp(r'x')));
  print('😀a😀'.split('😀'));
  print('😀a😀'.split('a'));

  print('|' + '  \t\n x \n\t '.trim() + '|');
  print('|' + '  x  '.trimLeft() + '|');
  print('|' + '  x  '.trimRight() + '|');
  print('|' + '\u00A0x\u00A0'.trim() + '|');
  print('|' + '\u2003x\u2003'.trim() + '|');
  print('|' + '\u3000x\u3000'.trim() + '|');
  print('|' + '\uFEFFx\uFEFF'.trim() + '|');
  print('|' + '\u200Bx\u200B'.trim() + '|');
  print('|' + '\u0085x\u0085'.trim() + '|');
  print('|' + '\r\n x \r\n'.trim() + '|');
  print('|' + '\u000Bx\u000C'.trim() + '|');
  print(''.trim().isEmpty);
  print('   '.trim().isEmpty);
  print('x'.trim() == 'x');
  print('  x  '.trim().length);
  print('  x  '.trimLeft().length);
  print('  x  '.trimRight().length);

  print('7'.padLeft(3, '0'));
  print('7'.padLeft(5, 'ab'));
  print('7'.padLeft(6, 'ab'));
  print('7'.padRight(5, 'ab'));
  print('7'.padRight(6, 'xyz'));
  print('7'.padLeft(0, '0'));
  print('7'.padLeft(-1, '0'));
  print('7'.padLeft(3, ''));
  print('abc'.padLeft(2, '*'));
  print('|' + ''.padLeft(3) + '|');
  print('|' + ''.padRight(3) + '|');
  print('x'.padLeft(3, '😀'));
  print('x'.padLeft(3, '😀').length);
  print('x'.padLeft(3, 'é'));
  for (var i = 1; i <= 4; i++) {
    print(i.toString().padLeft(3, '.') + '|' + ('*' * i).padRight(5, '-'));
  }

  print('abc'.replaceAll('', '-'));
  print(''.replaceAll('', '-'));
  print('abc'.replaceFirst('', '-'));
  print('abc'.replaceAll(RegExp(''), '-'));
  print('abc'.replaceAll('b', ''));
  print('aaa'.replaceAll('aa', 'b'));
  print('aaaa'.replaceAll('aa', 'b'));
  print('abcabc'.replaceAll('abc', ''));
  print('abc'.replaceAll('abc', 'abcabc'));
  print('a.b.c'.replaceAll('.', '..'));
  print('abc'.replaceRange(0, 0, 'X'));
  print('abc'.replaceRange(3, 3, 'X'));
  print('abc'.replaceRange(0, 3, ''));
  print('abc'.replaceFirst('c', 'C', 2));
  print('abcabc'.replaceFirst('abc', 'X', 1));

  print('a1b2c3'.splitMapJoin(RegExp(r'\d')));
  print('a1b2c3'.splitMapJoin(RegExp(r'\d'), onMatch: (m) => '[${m[0]}]'));
  print('a1b2c3'.splitMapJoin(RegExp(r'\d'), onNonMatch: (s) => s.toUpperCase()));
  print('a1b2c3'.splitMapJoin(RegExp(r'\d'), onMatch: (m) => '', onNonMatch: (s) => s));
  print('a1b2c3'.splitMapJoin('2', onMatch: (m) => '<${m[0]}>', onNonMatch: (s) => '($s)'));
  print(''.splitMapJoin('x', onMatch: (m) => 'M', onNonMatch: (s) => 'N'));
  print('xx'.splitMapJoin('x', onMatch: (m) => 'M', onNonMatch: (s) => 'N'));
  print('abc'.splitMapJoin(RegExp(r'(?=b)'), onMatch: (m) => '|', onNonMatch: (s) => s));

  var s = 'abcabcabc';
  print(s.indexOf('abc'));
  print(s.indexOf('abc', 1));
  print(s.indexOf('abc', 3));
  print(s.indexOf('abc', 4));
  print(s.indexOf('abc', 7));
  print(s.indexOf('abc', 9));
  print(s.indexOf('', 9));
  print(s.indexOf('', 4));
  print(s.lastIndexOf('abc'));
  print(s.lastIndexOf('abc', 5));
  print(s.lastIndexOf('abc', 6));
  print(s.lastIndexOf('abc', 0));
  print(s.lastIndexOf('abc', 2));
  print(s.lastIndexOf('', 3));
  print(s.indexOf(RegExp('c.'), 3));
  print(s.lastIndexOf(RegExp('a')));
  print(s.indexOf('z', 0));
  print('aaa'.indexOf('aa', 1));
  print('aaa'.lastIndexOf('aa'));
  var conta = 0;
  var pos = s.indexOf('bc');
  while (pos != -1) {
    conta++;
    pos = s.indexOf('bc', pos + 1);
  }
  print(conta);
  print('abc'.startsWith('', 3));
  print('abc'.startsWith('c', 2));
  print('abc'.startsWith(RegExp('b'), 1));
  print('abc'.endsWith(''));
  print('a b c'.split(' ').map((w) => w.padLeft(2, '_')).join(''));
  print('k=v;a=b'.split(';').map((p) => p.split('=')).toList());
  print('  a , b ,c  '.split(',').map((e) => e.trim()).where((e) => e.isNotEmpty).toList());
  print('linha1\nlinha2\r\nlinha3'.split(RegExp(r'\r?\n')));
}
