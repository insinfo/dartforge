// codeUnits, runes, Runes, fromCharCode(s), codeUnitAt, iteração reversa por runes e contagem de graphemes simples.
void main() {
  var s = 'héllo 😀!';
  print(s.length);
  print(s.codeUnits);
  print(s.codeUnits.length);
  print(s.runes.toList());
  print(s.runes.length);
  print(s.codeUnitAt(0));
  print(s.codeUnitAt(1));
  print(s.codeUnitAt(6));
  print(s.codeUnitAt(7));
  print(s.codeUnitAt(s.length - 1));
  print(s.runes.first);
  print(s.runes.last);
  print(s.runes.elementAt(6));
  print(s.runes.contains(0x1F600));
  print(s.runes.contains(0xD83D));
  print(s.codeUnits.contains(0xD83D));
  print(s.runes.string);
  print(s.runes.string == s);
  print(s.runes is Runes);
  print(Runes('ab').toList());
  print(Runes('😀').length);
  print(s.runes.where((r) => r > 127).map((r) => r.toRadixString(16)).toList());
  print(s.runes.map((r) => r < 128 ? 'A' : 'U').join());
  print(s.codeUnits.map((c) => c > 0xD7FF && c < 0xE000 ? 'S' : '.').join());

  var it = s.runes.iterator;
  var vistos = <String>[];
  while (it.moveNext()) {
    vistos.add('${it.rawIndex}:${it.currentSize}');
  }
  print(vistos);
  var it2 = s.runes.iterator;
  it2.moveNext();
  print(it2.current);
  print(it2.currentAsString);
  print(it2.rawIndex);
  it2.reset(6);
  it2.moveNext();
  print(it2.currentAsString);
  print(it2.currentSize);
  var rev = <String>[];
  var itr = RuneIterator.at(s, s.length);
  while (itr.movePrevious()) {
    rev.add(itr.currentAsString);
  }
  print(rev.join());
  print(rev.length);
  print(String.fromCharCodes(s.runes.toList().reversed));
  print(String.fromCharCodes(s.codeUnits.reversed).length);

  print(String.fromCharCode(65));
  print(String.fromCharCode(0xE9));
  print(String.fromCharCode(0x1F600));
  print(String.fromCharCode(0x1F600).length);
  print(String.fromCharCodes([72, 105, 33]));
  print(String.fromCharCodes([0x48, 0x69], 1));
  print(String.fromCharCodes([1, 2, 3, 4, 5].map((i) => 96 + i)));
  print(String.fromCharCodes(Iterable.generate(5, (i) => 65 + i)));
  print(String.fromCharCodes([0xD83D, 0xDE00]) == '😀');
  print(String.fromCharCodes([0xD83D]).length);
  print(String.fromCharCodes([]).isEmpty);
  print(String.fromCharCodes('abc'.codeUnits) == 'abc');
  print(String.fromCharCodes('abc'.runes) == 'abc');
  print(String.fromCharCodes(s.runes) == s);

  print('a'.codeUnitAt(0) - 'A'.codeUnitAt(0));
  print('Z'.codeUnitAt(0) - 'A'.codeUnitAt(0) + 1);
  print('0'.codeUnitAt(0));
  print('9'.codeUnitAt(0));
  print(' '.codeUnitAt(0));
  print('\n'.codeUnitAt(0));
  print('~'.codeUnitAt(0));
  print('\u007F'.codeUnitAt(0));
  print('\u0080'.codeUnitAt(0));
  print('\uFFFF'.codeUnitAt(0));
  print('\u{10000}'.codeUnitAt(0));
  print('\u{10000}'.codeUnitAt(1));
  print('\u{10FFFF}'.codeUnits);
  try {
    'abc'.codeUnitAt(3);
  } catch (e) {
    print('lançou ${e is RangeError}');
  }
  var cesar = String.fromCharCodes('hello'.codeUnits.map((c) => (c - 97 + 3) % 26 + 97));
  print(cesar);
  var upper = String.fromCharCodes('hello'.codeUnits.map((c) => c - 32));
  print(upper);
  var freq = <int, int>{};
  for (var c in 'banana'.codeUnits) {
    freq[c] = (freq[c] ?? 0) + 1;
  }
  var chaves = freq.keys.toList()..sort();
  print(chaves.map((k) => '${String.fromCharCode(k)}=${freq[k]}').join(' '));

  int graphemesSimples(String texto) {
    var n = 0;
    for (var r in texto.runes) {
      if (r >= 0x300 && r <= 0x36F) continue;
      if (r == 0xFE0F || r == 0x200D) continue;
      if (r >= 0x1F3FB && r <= 0x1F3FF) continue;
      n++;
    }
    return n;
  }

  print(graphemesSimples('abc'));
  print(graphemesSimples('e\u0301'));
  print(graphemesSimples('é'));
  print(graphemesSimples('👍🏽'));
  print(graphemesSimples('a😀b'));
  print(graphemesSimples(''));
  print('e\u0301'.length);
  print('e\u0301'.runes.length);
  print('👍🏽'.length);
  print('👍🏽'.runes.length);
  print('a😀b'.length);
  print('a😀b'.runes.length);
  print(s.runes.toList().map((r) => String.fromCharCode(r)).toList());
  print(s.runes.skip(6).take(1).map((r) => r.toRadixString(16)).toList());
  print(s.codeUnits.sublist(6, 8).map((c) => c.toRadixString(16)).toList());
  print(s.codeUnits.indexOf(0xDE00));
  print(s.runes.toList().indexOf(0x1F600));
  print(s.codeUnits.reduce((a, b) => a > b ? a : b));
  print(s.runes.reduce((a, b) => a > b ? a : b));
  print(s.codeUnits.fold<int>(0, (a, b) => a + b));
  print(s.runes.fold<int>(0, (a, b) => a + b));
}
