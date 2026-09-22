// Pares substitutos: length/codeUnits/runes/codeUnitAt de emoji, fromCharCode(s), substring e iteração por runes.
void main() {
  var emoji = '😀';
  print(emoji.length);
  print(emoji.codeUnits);
  print(emoji.runes.toList());
  print(emoji.codeUnitAt(0));
  print(emoji.codeUnitAt(1));
  print(emoji.runes.length);
  print(emoji.runes.first);
  print(emoji.runes.first.toRadixString(16));

  var misto = 'a😀b';
  print(misto.length);
  print(misto.runes.length);
  print(misto.codeUnits);
  print(misto.runes.toList());
  print(misto.substring(0, 1));
  print(misto.substring(3));
  print(misto.substring(1, 3) == emoji);
  print(misto.substring(1, 2).length);
  print(misto.substring(1, 2).codeUnitAt(0));

  print(String.fromCharCode(0x1F600) == emoji);
  print(String.fromCharCode(0x1F600).length);
  print(String.fromCharCodes([0xD83D, 0xDE00]) == emoji);
  print(String.fromCharCodes([97, 0x1F600, 98]) == misto);
  print(String.fromCharCode(0xD83D).length);
  print(String.fromCharCode(0xD83D).codeUnitAt(0));

  var clave = '\u{1D11E}';
  print(clave.length);
  print(clave.codeUnits.map((c) => c.toRadixString(16)).join(' '));
  print(clave.runes.first);

  for (var r in 'hé😀'.runes) {
    print('$r ${String.fromCharCode(r)}');
  }
  var buf = StringBuffer();
  for (var r in 'x😀y'.runes.toList().reversed) {
    buf.write(String.fromCharCode(r));
  }
  print(buf.toString() == 'y😀x');
  print('😀😀😀'.length);
  print('😀😀😀'.runes.length);
  print('😀😀😀'.indexOf('😀', 1));
  print('😀😀😀'.lastIndexOf('😀'));
  print('a😀b'.split('').length);
  print('👍🏽'.runes.length);
  print('👍🏽'.length);
  print('é'.runes.length);
  print('e\u0301'.runes.length);
  print('e\u0301'.length);
  print('😀'.codeUnits.map((c) => c.toRadixString(16)).toList());
  print('😀'.runes.map((r) => r.toRadixString(16)).toList());
  print(String.fromCharCodes('😀'.codeUnits) == '😀');
  print(String.fromCharCodes('😀'.runes) == '😀');
  print('a😀b'.contains('😀'));
  print('a😀b'.indexOf('😀'));
  print('a😀b'.replaceAll('😀', '-'));
  print('😀'.padLeft(4, '.'));
  print('😀'.padLeft(4, '.').length);
}
