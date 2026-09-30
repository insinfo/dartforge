// Identidade e hash de strings no espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md
// §2.10, §2.11, §5.3): `identical` de literais iguais entre bibliotecas
// (textos_parte.dart) e em `const`, de uma string montada em tempo de execução
// (falso), o `hashCode` de conteúdo igual ao da VM (StringHasher) e estável,
// `==` e `hashCode` entre as formas de um e de dois bytes, strings como chaves
// de `Map`/`Set` (100 mil) e `switch` sobre string.
import 'textos_parte.dart';
import 'textos_parte.dart' as parte;

const localConst = 'olá, mundo';
const compostoConst = 'olá, ' 'mundo';
const interpConst = 'olá, ${'mundo'}';

class Portador {
  final String s;
  const Portador(this.s);
}

String classifica(String s) {
  switch (s) {
    case 'zero':
      return 'Z';
    case 'um':
    case 'uno':
      return 'U';
    case 'olá, mundo':
      return 'saudação';
    case 'ok 😀':
      return 'emoji';
    case '':
      return 'vazio';
    default:
      return 'outro(${s.length})';
  }
}

String classificaExpr(String s) => switch (s) {
      'ÿ' => 'latin-1',
      'Ÿ' => 'dois bytes',
      String t when t.startsWith('p') => 'prefixo p',
      _ => 'nada',
    };

int resumoDeHash(Iterable<String> ss) {
  var h = 0;
  for (final s in ss) {
    h = (h * 1000003 + s.hashCode) & 0x7FFFFFFF;
  }
  return h;
}

void main() {
  // identical de literais e constantes.
  print('literal == const da parte: ${identical('olá, mundo', literalConst)}');
  print('const local x const da parte: ${identical(localConst, parte.literalConst)}');
  print('final da parte: ${identical(localConst, literalFinal)}');
  print('composto const: ${identical(localConst, compostoConst)}');
  print('interpolação const: ${identical(localConst, interpConst)}');
  print('emoji const: ${identical('ok 😀', emojiConst)} ${identical(emojiConst, emojiDeFuncao())}');
  print('literal de função: ${identical('texto compartilhado', literalDeFuncao())}');
  print('vazio: ${identical('', vazioDeFuncao())}');
  print('lista const: ${identical(listaConst[2], localConst)} ${identical(listaConst, const ['a', 'b', 'olá, mundo'])}');
  print('objeto const: ${identical(const Portador('olá, mundo').s, localConst)} '
      '${identical(const Portador('x'), const Portador('x'))}');

  // Strings montadas em tempo de execução.
  final montada = montadoNaParte(1);
  print('montada == literal: ${montada == literalDeFuncao()}');
  print('montada identical literal: ${identical(montada, literalDeFuncao())}');
  final b = StringBuffer()..write('olá, ')..write('mundo');
  final doBuffer = b.toString();
  print('buffer ==: ${doBuffer == localConst} identical: ${identical(doBuffer, localConst)}');
  final sub = 'xxolá, mundoxx'.substring(2, 12);
  print('substring ==: ${sub == localConst} identical: ${identical(sub, localConst)}');
  final mesma = montada;
  print('mesma variável: ${identical(mesma, montada)}');

  // hashCode igual ao da VM.
  const amostras = [
    '',
    'a',
    'b',
    'abc',
    'hello world',
    'olá, mundo',
    'ação',
    'ÿ',
    'Ā',
    '日本語',
    'ok 😀',
    '😀',
    '\u0000',
    'The quick brown fox jumps over the lazy dog',
  ];
  for (final s in amostras) {
    print('hash "${s.replaceAll('\u0000', r'\0')}": ${s.hashCode}');
  }
  print('hash longo: ${('abcdefghij' * 1000).hashCode}');
  print('hash estável: ${montada.hashCode == montada.hashCode} ${montada.hashCode == 'texto compartilhado'.hashCode}');
  print('identityHashCode de string: ${identityHashCode('abc') == 'abc'.hashCode} '
      '${identityHashCode(doBuffer) == doBuffer.hashCode}');

  // == e hashCode entre formas (dois bytes que volta a caber em um).
  final doisBytes = 'aĀ';
  final umDeDois = doisBytes.substring(0, 1);
  print('substring de dois bytes == "a": ${umDeDois == 'a'} hash: ${umDeDois.hashCode == 'a'.hashCode}');
  final deCodigos = String.fromCharCodes([0x61, 0xE7, 0xE3, 0x6F]);
  print('fromCharCodes == "ação": ${deCodigos == 'ação'}');
  final deDois = String.fromCharCodes([0x100, 0x61]).substring(1);
  print('fromCharCodes dois→um: ${deDois == 'a'} ${deDois.hashCode == 'a'.hashCode}');
  final minusc = 'ÀÉ'.toLowerCase();
  print('toLowerCase: ${minusc == 'àé'} ${minusc.hashCode == 'àé'.hashCode}');

  // Map e Set com 100 mil chaves string.
  const n = 100000;
  final mapa = <String, int>{};
  for (var i = 0; i < n; i++) {
    mapa['k$i'] = i;
  }
  final conjunto = <String>{};
  for (var i = 0; i < n; i++) {
    conjunto.add(i.isEven ? 'e$i' : 'ē$i');
  }
  print('mapa: ${mapa.length} conjunto: ${conjunto.length}');
  var faltas = 0;
  for (var i = 0; i < n; i++) {
    final k = StringBuffer('k')..write(i);
    if (mapa[k.toString()] != i) faltas++;
    if (!conjunto.contains('${i.isEven ? 'e' : 'ē'}$i')) faltas++;
  }
  print('faltas: $faltas');
  print('primeiras chaves: ${mapa.keys.take(3).toList()} últimas: ${conjunto.skip(n - 3).toList()}');
  for (var i = 0; i < n; i += 3) {
    mapa.remove('k$i');
    conjunto.remove(i.isEven ? 'e$i' : 'ē$i');
  }
  print('após remover: ${mapa.length} ${conjunto.length} ${mapa['k3']} ${mapa['k4']}');
  print('resumo de hash das chaves: ${resumoDeHash(mapa.keys.take(1000))}');
  final ident = Map<String, int>.identity();
  ident[localConst] = 1;
  ident[doBuffer] = 2;
  print('Map.identity: ${ident.length} ${ident[literalConst]} ${ident['olá, mundo']}');

  // switch sobre string.
  for (final s in ['zero', 'um', 'uno', literalConst, doBuffer, emojiDeFuncao(), '', 'outra coisa']) {
    print('switch "$s": ${classifica(s)}');
  }
  for (final s in ['ÿ', 'ÿ'.toUpperCase(), 'pera', 'uva']) {
    print('switch expr "$s": ${classificaExpr(s)}');
  }
}
