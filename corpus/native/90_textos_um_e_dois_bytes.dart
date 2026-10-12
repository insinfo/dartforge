// Strings de um e de dois bytes no espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md
// §5.3): a forma latin-1 (_OneByteString) até 0xFF e a de 16 bits (_TwoByteString)
// a partir de 0x100, CJK e emoji, pares substitutos inteiros e soltos, `substring`
// que corta um par ao meio, `codeUnitAt`, `runes`, `toUpperCase`/`toLowerCase` que
// trocam de forma ('ÿ' → 'Ÿ' U+0178, 'µ' → 'Μ' U+039C; 'ß' fica 'ß' na VM, que não
// expande para 'SS') e o `print` de um substituto solto (sai como U+FFFD).

String hex(int c) => c.toRadixString(16).padLeft(4, '0');

String unidades(String s) => s.codeUnits.map(hex).join(' ');

String runas(String s) => s.runes.map((r) => r.toRadixString(16)).join(' ');

void mostrar(String nome, String s) {
  print('$nome: len=${s.length} runas=${s.runes.length} [${unidades(s)}]');
}

void tentar(String nome, Object? Function() f) {
  try {
    print('$nome: ${f()}');
  } catch (e) {
    print('$nome: ${e.runtimeType}');
  }
}

void main() {
  // Limite do latin-1.
  final ff = String.fromCharCode(0xFF);
  final cem = String.fromCharCode(0x100);
  mostrar('0xFF', ff);
  mostrar('0x100', cem);
  final misto = 'abc${ff}def';
  mostrar('misto um byte', misto);
  final misto2 = misto + cem;
  mostrar('misto dois bytes', misto2);
  print('ff == ÿ: ${ff == 'ÿ'}');
  print('cem == Ā: ${cem == 'Ā'}');
  print('compareTo ff/cem: ${ff.compareTo(cem)} ${cem.compareTo(ff)}');

  // Todas as unidades de 0xF0 a 0x10F montadas uma a uma.
  final faixa = String.fromCharCodes(List.generate(32, (i) => 0xF0 + i));
  mostrar('faixa', faixa);
  print('faixa até 0xFF: ${unidades(faixa.substring(0, 16))}');
  print('faixa depois: ${unidades(faixa.substring(16))}');
  print('indexOf 0x100: ${faixa.indexOf(cem)}');

  // CJK e emoji.
  const cjk = '日本語テキスト';
  mostrar('cjk', cjk);
  print(cjk);
  print('cjk[2]: ${cjk[2]} codeUnitAt(2)=${hex(cjk.codeUnitAt(2))}');
  const emoji = 'a😀b🎉c';
  mostrar('emoji', emoji);
  print(emoji);
  print('runas: ${runas(emoji)}');
  print('emoji.codeUnitAt(1)=${hex(emoji.codeUnitAt(1))} codeUnitAt(2)=${hex(emoji.codeUnitAt(2))}');
  final construido = String.fromCharCodes([0x1F600, 0x41, 0x1F389]);
  mostrar('fromCharCodes com runas', construido);
  print('construido == emoji montado: ${construido == '😀A🎉'}');
  print('String.fromCharCode(0x1F600): ${String.fromCharCode(0x1F600) == '😀'}');

  // Substring no meio do par.
  final metade1 = emoji.substring(0, 2); // 'a' + alto solto
  final metade2 = emoji.substring(2, 4); // baixo solto + 'b'
  mostrar('metade1', metade1);
  mostrar('metade2', metade2);
  print('runas metade1: ${runas(metade1)}');
  print('runas metade2: ${runas(metade2)}');
  final junta = metade1 + metade2;
  mostrar('junta', junta);
  print('junta == a😀b: ${junta == 'a😀b'}');
  print('runas junta: ${runas(junta)}');

  // Substitutos soltos montados à mão.
  final alto = String.fromCharCode(0xD83D);
  final baixo = String.fromCharCode(0xDE00);
  mostrar('alto', alto);
  mostrar('baixo', baixo);
  print('runas baixo+alto: ${runas(baixo + alto)}');
  print('alto+baixo == 😀: ${alto + baixo == '😀'}');
  print('print de solto:');
  print(alto);
  print('x${baixo}y');
  print('solto num par invertido: ${baixo + alto}');

  // Iteração por runas com RuneIterator.
  final it = RuneIterator('x😀y');
  final passos = <String>[];
  while (it.moveNext()) {
    passos.add('${it.rawIndex}:${it.current.toRadixString(16)}/${it.currentSize}');
  }
  print('RuneIterator: ${passos.join(' ')}');

  // Mudança de forma em maiúsculas e minúsculas.
  const yTrema = 'ÿ';
  final yMai = yTrema.toUpperCase();
  mostrar('ÿ.toUpperCase', yMai);
  print(yMai);
  print('volta: ${yMai.toLowerCase() == yTrema}');
  const ss = 'straße';
  final ssMai = ss.toUpperCase();
  mostrar('straße.toUpperCase', ssMai);
  print(ssMai);
  const micro = 'µ';
  final microMai = micro.toUpperCase();
  mostrar('µ.toUpperCase', microMai);
  print('µ upper lower: ${unidades(microMai.toLowerCase())}');
  const acentos = 'àéîõüç ÀÉÎÕÜÇ';
  print('acentos upper: ${acentos.toUpperCase()}');
  print('acentos lower: ${acentos.toLowerCase()}');
  const grego = 'ΑΒΓ δεζ';
  print('grego upper: ${grego.toUpperCase()} lower: ${grego.toLowerCase()}');
  const cirilico = 'Привет';
  print('cirílico upper: ${cirilico.toUpperCase()} lower: ${cirilico.toLowerCase()}');
  print('emoji upper: ${emoji.toUpperCase()}');
  final dois = 'ĀBC';
  final um = dois.toLowerCase();
  mostrar('ĀBC.toLowerCase', um);

  // Operações que atravessam as duas formas.
  print('replaceAll: ${'aÿbÿc'.replaceAll('ÿ', 'Ā')}');
  print('split: ${'x😀y😀z'.split('😀')}');
  print('indexOf baixo: ${emoji.indexOf(baixo)} lastIndexOf alto: ${emoji.lastIndexOf(String.fromCharCode(0xD83C))}');
  print('contains: ${emoji.contains('🎉')} ${cjk.contains('語')}');
  final espacos = '${String.fromCharCode(0xA0)} ÿ ${String.fromCharCode(0x3000)}';
  print('trim: [${espacos.trim()}] len=${espacos.trim().length}');
  print('padLeft dois bytes: ${'Ā'.padLeft(4, 'ÿ')}');
  print('* dois bytes: ${'日'.padRight(3, '-') * 2}');
  print('split vazio: ${'a😀'.split('').map(unidades).toList()}');
  print('compareTo emoji/cjk: ${emoji.compareTo(cjk)} ${String.fromCharCode(0xFFFF).compareTo('😀')}');
  print('hash iguais: ${junta.hashCode == 'a😀b'.hashCode} ${(('a' * 3) + 'Ā').hashCode == 'aaaĀ'.hashCode}');

  // Erros de índice.
  tentar('codeUnitAt(-1)', () => 'ÿ'.codeUnitAt(-1));
  tentar('codeUnitAt(len)', () => emoji.codeUnitAt(emoji.length));
  tentar('substring invertido', () => cjk.substring(3, 1));
  tentar('fromCharCode(0x110000)', () => String.fromCharCode(0x110000));
}
