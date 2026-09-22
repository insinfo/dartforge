// RegExp: hasMatch, firstMatch/group(s), allMatches com start/end, grupos nomeados, replaceAll/Mapped, split, flags e escape.
void main() {
  var re = RegExp(r'\d+');
  print(re.hasMatch('abc123'));
  print(re.hasMatch('abc'));
  print(re.pattern);
  print(re.isCaseSensitive);
  print(re.isMultiLine);
  print(re.isUnicode);
  print(re.isDotAll);
  var m = re.firstMatch('ab12cd345');
  print(m != null);
  print(m![0]);
  print(m.group(0));
  print(m.start);
  print(m.end);
  print(m.input);
  print(m.groupCount);
  print(re.firstMatch('nada'));
  print(re.stringMatch('x99y'));
  print(re.stringMatch('xy'));

  var g = RegExp(r'(\w+)@(\w+)\.(\w+)');
  var gm = g.firstMatch('mail: ana@site.com!')!;
  print(gm.groupCount);
  print(gm.group(0));
  print(gm.group(1));
  print(gm.group(2));
  print(gm.group(3));
  print(gm.groups([1, 3]));
  print(gm.groups([0, 1, 2, 3]));
  print(gm[2]);
  print(gm.start);
  print(gm.end);
  var opt = RegExp(r'a(b)?c').firstMatch('ac')!;
  print(opt.group(1));
  print(opt.groups([1]));

  var todos = re.allMatches('a1 b22 c333 d');
  print(todos.length);
  for (var x in todos) {
    print('${x[0]} [${x.start}, ${x.end})');
  }
  print(todos.map((x) => x[0]).toList());
  print(re.allMatches('sem').isEmpty);
  print(re.allMatches('a1b2', 2).map((x) => x[0]).toList());
  print(RegExp(r'').allMatches('ab').length);
  print(RegExp(r'a*').allMatches('baab').map((x) => '${x.start}:${x[0]}').toList());
  print(RegExp(r'(a)(b)?').allMatches('a ab').map((x) => x.groups([1, 2])).toList());

  var nomeado = RegExp(r'(?<ano>\d{4})-(?<mes>\d{2})-(?<dia>\d{2})');
  var nm = nomeado.firstMatch('data: 2024-02-29')!;
  print(nm.namedGroup('ano'));
  print(nm.namedGroup('mes'));
  print(nm.namedGroup('dia'));
  print(nm.groupNames.toList());
  print(nm.groupCount);
  print(nm.group(1));
  try {
    nm.namedGroup('nao');
  } catch (e) {
    print('lançou ${e is ArgumentError}');
  }

  print('a1b22c333'.replaceAll(re, '#'));
  print('a1b22c333'.replaceFirst(re, '#'));
  print('a1b22c333'.replaceAllMapped(re, (x) => '<${x[0]!.length}>'));
  print('a1b22c333'.replaceFirstMapped(re, (x) => '(${x[0]})'));
  print('ana@site.com'.replaceAllMapped(g, (x) => '${x[3]}.${x[2]}@${x[1]}'));
  print('hello world'.replaceAll(RegExp(r'o'), '0'));
  print('hello world'.replaceAll(RegExp(r'(l+)'), r'[$1]'));
  print('aaa'.replaceAll(RegExp(r'a'), r'$0$0'));
  print('x'.replaceAll(RegExp(r'x'), r'$'));
  print('abc'.replaceAll(RegExp(r'^'), '>'));
  print('abc'.replaceAll(RegExp(r'$'), '<'));
  print('a.b.c'.replaceAll(RegExp(r'\.'), '/'));
  print('CamelCaseString'.replaceAllMapped(RegExp(r'([A-Z])'), (x) => '_${x[1]!.toLowerCase()}'));

  print('a1b22c333'.split(re));
  print('a, b,c ,d'.split(RegExp(r'\s*,\s*')));
  print('  x  y  '.trim().split(RegExp(r'\s+')));
  print('abc'.split(RegExp(r'')));
  print('a1b'.split(RegExp(r'\d')));
  print('1a2'.split(RegExp(r'\d')));
  print(''.split(RegExp(r'x')));
  print('aXbXc'.split(RegExp('x', caseSensitive: false)));
  print('a\nb\nc'.split(RegExp(r'\n')).length);

  print(RegExp('abc', caseSensitive: false).hasMatch('ABC'));
  print(RegExp('abc').hasMatch('ABC'));
  print(RegExp(r'^b', multiLine: true).allMatches('a\nb\nb').length);
  print(RegExp(r'^b').allMatches('a\nb\nb').length);
  print(RegExp(r'a.b', dotAll: true).hasMatch('a\nb'));
  print(RegExp(r'a.b').hasMatch('a\nb'));
  print(RegExp(r'^.$', unicode: true).hasMatch('😀'));
  print(RegExp(r'^.$').hasMatch('😀'));
  print(RegExp(r'^..$').hasMatch('😀'));
  print(RegExp(r'\u{1F600}', unicode: true).hasMatch('😀'));
  print(RegExp(r'\p{L}+', unicode: true).firstMatch('123 ação 4')![0]);
  print(RegExp(r'[a-z]+', caseSensitive: false).allMatches('Ab cD').map((x) => x[0]).toList());

  print(RegExp.escape('a.b*c?d(e)[f]{g}|h^i\$j\\k+l'));
  print(RegExp(RegExp.escape('1+1=2')).hasMatch('1+1=2'));
  print(RegExp(RegExp.escape('1+1=2')).hasMatch('11=2'));
  print(RegExp.escape('sem especiais'));
  print(RegExp(r'\bfoo\b').hasMatch('a foo b'));
  print(RegExp(r'\bfoo\b').hasMatch('afoob'));
  print(RegExp(r'(\d)\1').hasMatch('11'));
  print(RegExp(r'(\d)\1').hasMatch('12'));
  print(RegExp(r'a(?=b)').firstMatch('ab')?[0]);
  print(RegExp(r'a(?!b)').firstMatch('ab'));
  print(RegExp(r'(?<=x)y').firstMatch('xy')?[0]);
  print(RegExp(r'colou?r').hasMatch('color') && RegExp(r'colou?r').hasMatch('colour'));
  print(RegExp(r'^\d{3}-\d{4}$').hasMatch('555-1234'));
  print(RegExp(r'^[\w.]+@[\w.]+\.\w+$').hasMatch('a.b@c.d.com'));
  print(RegExp(r'x{2,3}').allMatches('x xx xxx xxxx').map((x) => x[0]).toList());
  print(RegExp(r'a|b').allMatches('cab').length);
  print(RegExp(r'a+?').firstMatch('aaa')![0]);
  print(RegExp(r'a+').firstMatch('aaa')![0]);
  try {
    RegExp('(');
  } catch (e) {
    print('lançou ${e is FormatException}');
  }
  print(RegExp(r'a').matchAsPrefix('abc')?[0]);
  print(RegExp(r'b').matchAsPrefix('abc'));
  print(RegExp(r'b').matchAsPrefix('abc', 1)?[0]);
  print('abc'.contains(RegExp('B', caseSensitive: false)));
  print('abc'.startsWith(RegExp('a.')));
  print('abc'.indexOf(RegExp('[bc]')));
  print('abcbc'.lastIndexOf(RegExp('bc')));
}
