// Strings grandes no espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §5.3):
// concatenação e interpolação que atravessam as classes de tamanho (pequenas,
// ~2 KiB, ~16 KiB, ~64 KiB) até a região grande (1 MiB), um `StringBuffer` de
// ~5 MiB, o operador `*`, `padLeft`, `split`/`join` grandes e `writeCharCode`
// de um par substituto. Imprime comprimentos, resumos determinísticos das
// unidades e trechos — nunca a string inteira.

/// Resumo das unidades: soma ponderada módulo um primo (independe do hashCode).
int resumo(String s) {
  var h = 0;
  for (var i = 0; i < s.length; i++) {
    h = (h * 31 + s.codeUnitAt(i)) % 1000000007;
  }
  return h;
}

String trecho(String s) {
  if (s.length <= 24) return s;
  return '${s.substring(0, 12)}…${s.substring(s.length - 12)}';
}

void mostrar(String nome, String s) {
  print('$nome: len=${s.length} resumo=${resumo(s)} [${trecho(s)}]');
}

void main() {
  // Concatenação dobrando de tamanho: passa por todas as classes.
  var s = 'ab';
  final marcos = {16, 2048, 16384, 65536, 1 << 20};
  while (s.length < (1 << 20)) {
    s = s + s;
    if (marcos.contains(s.length)) mostrar('dobra ${s.length}', s);
  }

  // Concatenação por passos pequenos (muitas strings intermediárias).
  var acum = '';
  for (var i = 0; i < 3000; i++) {
    acum += '${i % 10}';
    if (acum.length == 2048) mostrar('passos 2048', acum);
  }
  mostrar('passos 3000', acum);

  // Interpolação de pedaços de tamanhos diversos.
  final k2 = 'x' * 2000;
  final k16 = 'y' * 16000;
  final k64 = 'z' * 64000;
  final i1 = '[$k2|${k16.length}|$k2]';
  mostrar('interp 2K', i1);
  final i2 = '<$k16>$k64<$k16>';
  mostrar('interp 96K', i2);
  final i3 = '${i2}Ā${i2}';
  mostrar('interp dois bytes', i3);
  print('dois bytes no meio: ${i3.codeUnitAt(i2.length).toRadixString(16)}');
  var mega = '';
  for (var i = 0; i < 16; i++) {
    mega = '$mega$k64${i.toRadixString(16)}';
  }
  mostrar('interp 1M', mega);

  // Operador *.
  mostrar('* 1', 'abc' * 1);
  mostrar('* 0', 'abc' * 0);
  mostrar('* 700', 'abc' * 700);
  mostrar('* dois bytes', 'Āb' * 40000);
  mostrar('* emoji', '😀' * 100000);

  // padLeft/padRight grandes.
  mostrar('padLeft', '42'.padLeft(100000, '0'));
  mostrar('padRight', 'fim'.padRight(70000, '.-'));
  mostrar('padLeft dois bytes', 'x'.padLeft(20000, 'Ω'));

  // StringBuffer de ~5 MiB, com write, writeCharCode e writeAll.
  final sb = StringBuffer();
  var linhas = 0;
  while (sb.length < 5 * 1024 * 1024) {
    sb.write('linha ');
    sb.write(linhas);
    sb.writeCharCode(0x3A);
    if (linhas % 1000 == 0) sb.writeCharCode(0x1F600); // par substituto
    sb.writeAll(['a', 'b', linhas % 7], ',');
    sb.writeln();
    linhas++;
  }
  final grande = sb.toString();
  mostrar('buffer', grande);
  print('linhas: $linhas');
  print('buffer runas no início: ${grande.substring(0, 20).runes.map((r) => r.toRadixString(16)).join(' ')}');

  // writeCharCode de par num buffer curto.
  final sb2 = StringBuffer('<');
  sb2.writeCharCode(0x1F389);
  sb2.writeCharCode(0x10FFFF);
  sb2.writeCharCode(0xFF);
  sb2.write('>');
  final curto = sb2.toString();
  print('curto: len=${curto.length} unidades=${curto.codeUnits.map((c) => c.toRadixString(16)).join(' ')}');

  // split e join grandes.
  final partes = grande.split('\n');
  print('split: ${partes.length} partes, última vazia: ${partes.last.isEmpty}');
  print('parte 1000: ${partes[1000]}');
  print('parte ${linhas - 1}: ${partes[linhas - 1]}');
  final junta = partes.join('\n');
  print('join igual ao original: ${junta == grande} ${junta.length}');
  final pedacos = List.generate(50000, (i) => 'p$i');
  final unida = pedacos.join(';');
  mostrar('join 50000', unida);
  final devolta = unida.split(';');
  print('split volta: ${devolta.length} ${devolta[49999]} ${devolta[12345]}');
  final porLetra = ('abcd' * 5000).split('');
  print('split vazio: ${porLetra.length} ${porLetra[4999]}');

  // Operações sobre o mega: substring na região grande, indexOf, replaceAll.
  final meio = mega.substring(500000, 500040);
  mostrar('substring grande', meio);
  print('indexOf "a": ${mega.indexOf('a')} lastIndexOf "f": ${mega.lastIndexOf('f')}');
  final troca = mega.replaceAll('z', 'q');
  mostrar('replaceAll', troca);
  print('toUpperCase: ${resumo(i1.toUpperCase())}');
  print('igualdade grande: ${mega == (StringBuffer()..write(mega)).toString()} '
      '${mega.hashCode == ('' + mega).hashCode}');
  print('compareTo grande: ${mega.compareTo('$mega!')} ${('${mega}a').compareTo('${mega}b')}');
}
