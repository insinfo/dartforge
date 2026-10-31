// UTF-8 (`dart:convert`): encoding and decoding of ASCII, Latin-1, BMP and
// astral text, short and long, plus the chunked decoder and `allowMalformed`.
import 'dart:convert';
import 'dart:typed_data';

import '../performance/common.dart';

String repeat(String s, int n) {
  final sb = StringBuffer();
  for (var i = 0; i < n; i++) {
    sb.write(s);
    sb.write(i % 10);
  }
  return sb.toString();
}

int bytesSum(List<int> b) {
  var h = b.length;
  for (var i = 0; i < b.length; i += 13) {
    h = (h * 31 + b[i]) & 0x3FFFFFFF;
  }
  return h;
}

int textSum(String s) {
  var h = s.length;
  for (var i = 0; i < s.length; i += 13) {
    h = (h * 31 + s.codeUnitAt(i)) & 0x3FFFFFFF;
  }
  return h;
}

void main() {
  final ascii = repeat('The quick brown fox jumps over the lazy dog. ', 20000);
  final latin1 = repeat('Olá, ação, coração, pão, maçã, Über, façade. ', 20000);
  final cjk = repeat('漢字かな交じり文 한국어 텍스트 中文字符 ', 20000);
  final astral = repeat('emoji 😀🎉🚀 clef 𝄞 math 𝔸𝔹 ', 20000);
  final short = [for (var i = 0; i < 64; i++) 'key_$i é ${i * 7}'];
  final asciiBytes = utf8.encode(ascii);
  final latin1Bytes = utf8.encode(latin1);
  final cjkBytes = utf8.encode(cjk);
  final astralBytes = utf8.encode(astral);
  final malformed = Uint8List.fromList([...latin1Bytes.sublist(0, 4000), 0xFF, 0xC3, ...asciiBytes.sublist(0, 4000)]);

  for (final (name, s, b) in [
    ('ascii', ascii, asciiBytes),
    ('latin1', latin1, latin1Bytes),
    ('cjk', cjk, cjkBytes),
    ('astral', astral, astralBytes),
  ]) {
    print('$name: ${s.length} units, ${b.length} bytes, sum ${bytesSum(b)}, '
        'round trip ${utf8.decode(b) == s}');
  }
  print('malformed: ${textSum(utf8.decode(malformed, allowMalformed: true))}');

  measure('encode_ascii', () => bytesSum(utf8.encode(ascii)));
  measure('encode_latin1', () => bytesSum(utf8.encode(latin1)));
  measure('encode_cjk', () => bytesSum(utf8.encode(cjk)));
  measure('encode_astral', () => bytesSum(utf8.encode(astral)));
  measure('decode_ascii', () => textSum(utf8.decode(asciiBytes)));
  measure('decode_latin1', () => textSum(utf8.decode(latin1Bytes)));
  measure('decode_cjk', () => textSum(utf8.decode(cjkBytes)));
  measure('decode_astral', () => textSum(utf8.decode(astralBytes)));
  measure('decode_malformed', () {
    var h = 0;
    for (var i = 0; i < 50; i++) {
      h = (h + textSum(utf8.decode(malformed, allowMalformed: true))) & 0x3FFFFFFF;
    }
    return h;
  });
  measure('short_round_trip', () {
    var h = 0;
    for (var r = 0; r < 500; r++) {
      for (final s in short) {
        final b = utf8.encode(s);
        h = (h + b.length + utf8.decode(b).length) & 0x3FFFFFFF;
      }
    }
    return h;
  });
  measure('chunked_decode', () {
    final out = StringBuffer();
    final sink = utf8.decoder.startChunkedConversion(StringConversionSink.fromStringSink(out));
    for (var i = 0; i < cjkBytes.length; i += 1001) {
      final end = i + 1001 < cjkBytes.length ? i + 1001 : cjkBytes.length;
      sink.add(cjkBytes.sublist(i, end));
    }
    sink.close();
    return textSum(out.toString());
  });
}
