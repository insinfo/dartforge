// JSON (`dart:convert`): decoding and encoding of realistic documents —
// small (~1 KB, many times), medium (~100 KB) and large (~6 MB) — with nested
// objects, lists of `int` and `double`, strings with escapes and Unicode
// (including outside the BMP), `JsonEncoder.withIndent`, `json.fuse(utf8)`
// (bytes) and `jsonDecode` with a reviver.
//
// The documents come from a deterministic generator; the result of each
// kernel is a cheap checksum of the structure or of the text, and `main`
// prints, before the measurements, the full sum (every unit) of each encoded
// document and the round-trip check, to compare with the VM.
import 'dart:convert';

import 'common.dart';

/// Deterministic congruential generator (the same numbers on every executor).
class Generator {
  int _s;
  Generator(this._s);
  int next() => _s = (_s * 1103515245 + 12345) & 0x7fffffff;
  int below(int n) => next() % n;
}

const _words = [
  'alpha', 'beta', 'gamma', 'delta', 'epsilon', 'zeta', 'eta', 'theta', //
  'iota', 'kappa', 'lambda', 'mu', 'nu', 'xi', 'omicron', 'pi',
];

// Strings with what the encoder must escape and with Unicode of 2, 3 and 4
// bytes in UTF-8 (the last two with surrogate pairs).
const _special = [
  '"double" quotes and back \\ slash',
  'new\nline\ttab\rreturn',
  'control \u0001\u0008\u000c\u001f end',
  'accents: ção, ã, é, ü, ß',
  'CJK 中文字符 日本語 한국어',
  'emoji 😀🎉 and clef 𝄞 outside the BMP',
  'plain / slash and <tags> & entities',
  '',
];

String _text(Generator g) {
  final n = 1 + g.below(6);
  final sb = StringBuffer();
  for (var i = 0; i < n; i++) {
    if (i > 0) sb.write(' ');
    sb.write(_words[g.below(_words.length)]);
  }
  if (g.below(4) == 0) sb.write(_special[g.below(_special.length)]);
  return sb.toString();
}

double _real(Generator g) {
  switch (g.below(5)) {
    case 0:
      return g.below(1000000) / 1000; // 123.456
    case 1:
      return -g.below(100000) / 7; // repeating decimals
    case 2:
      return g.below(1000) * 1e-9; // negative exponent
    case 3:
      return g.below(1000) * 1.5e21; // positive exponent
    default:
      return g.below(100).toDouble(); // 42.0
  }
}

int _integer(Generator g) {
  switch (g.below(4)) {
    case 0:
      return g.below(100);
    case 1:
      return -g.below(1000000);
    case 2:
      return g.next() * g.next(); // up to ~2^62
    default:
      return g.below(1 << 20);
  }
}

Map<String, dynamic> _record(Generator g, int id) => {
      'id': id,
      'name': _text(g),
      'active': g.below(2) == 0,
      'score': _real(g),
      'null': null,
      'tags': [for (var i = 0; i < g.below(5); i++) _words[g.below(16)]],
      'values': [for (var i = 0; i < 8; i++) _integer(g)],
      'measures': [for (var i = 0; i < 6; i++) _real(g)],
      'address': {
        'street': _text(g),
        'number': g.below(5000),
        'city': _special[g.below(_special.length)],
        'geo': {'lat': _real(g), 'lng': _real(g)},
      },
      'history': [
        for (var i = 0; i < g.below(3); i++)
          {'when': 1700000000 + g.below(1000000), 'note': _text(g), 'ok': g.below(3) != 0},
      ],
    };

/// A document with `n` records and a header.
Map<String, dynamic> document(int seed, int n) {
  final g = Generator(seed);
  return {
    'version': 3,
    'generated': 'dartforge “bench” ✓',
    'records': [for (var i = 0; i < n; i++) _record(g, i)],
    'summary': {'total': n, 'factor': 0.1 + 0.2, 'empty': {}, 'empty_list': []},
  };
}

/// Checksum of every unit of a string.
int textSum(String s) {
  var h = s.length;
  for (var i = 0; i < s.length; i++) {
    h = (h * 31 + s.codeUnitAt(i)) & 0x3FFFFFFF;
  }
  return h;
}

/// Cheap checksum of a decoded value: walks the whole structure, but of each
/// string only the length and the first unit, and of each `double` the
/// clamped integer part of a thousand times the value.
int valueSum(Object? v) {
  if (v == null) return 7;
  if (v is bool) return v ? 11 : 13;
  if (v is int) return v & 0xFFFFFF;
  if (v is double) return (v * 1000).clamp(-1e9, 1e9).toInt() & 0xFFFFFF;
  if (v is String) return v.isEmpty ? 17 : v.length * 131 + v.codeUnitAt(0);
  if (v is List) {
    var h = 19 + v.length;
    for (final e in v) {
      h = (h * 31 + valueSum(e)) & 0x3FFFFFFF;
    }
    return h;
  }
  if (v is Map) {
    var h = 23 + v.length;
    v.forEach((k, e) {
      h = (h * 31 + valueSum(k) + valueSum(e)) & 0x3FFFFFFF;
    });
    return h;
  }
  throw StateError('unexpected type ${v.runtimeType}');
}

/// Cheap check of the encoded text: the length and one unit every 97.
int sample(String s) {
  var h = s.length;
  for (var i = 0; i < s.length; i += 97) {
    h = (h * 31 + s.codeUnitAt(i)) & 0x3FFFFFFF;
  }
  return h;
}

void main() {
  final smallObj = document(1, 2);
  final mediumObj = document(2, 200);
  final largeObj = document(3, 12000);
  final small = jsonEncode(smallObj);
  final medium = jsonEncode(mediumObj);
  final large = jsonEncode(largeObj);
  final mediumBytes = utf8.encode(medium);
  final indented = const JsonEncoder.withIndent('  ').convert(mediumObj);

  // Full check, outside the timing: sizes, sum of every unit and the round
  // trip (decoding and encoding again gives the same).
  for (final (name, s) in [('small', small), ('medium', medium), ('large', large), ('indented', indented)]) {
    print('$name: ${s.length} units, sum ${textSum(s)}, '
        'round trip ${jsonEncode(jsonDecode(s)) == (name == 'indented' ? medium : s)}');
  }
  print('bytes: ${mediumBytes.length}, ${json.fuse(utf8).decode(mediumBytes) is Map}');
  print('decoded: ${valueSum(jsonDecode(large))}');

  measure('decode_small', () {
    var h = 0;
    for (var i = 0; i < 2000; i++) {
      h = (h + valueSum(jsonDecode(small))) & 0x3FFFFFFF;
    }
    return h;
  });
  measure('encode_small', () {
    var h = 0;
    for (var i = 0; i < 2000; i++) {
      h = (h + sample(jsonEncode(smallObj))) & 0x3FFFFFFF;
    }
    return h;
  });
  measure('decode_medium', () {
    var h = 0;
    for (var i = 0; i < 30; i++) {
      h = (h + valueSum(jsonDecode(medium))) & 0x3FFFFFFF;
    }
    return h;
  });
  measure('encode_medium', () {
    var h = 0;
    for (var i = 0; i < 30; i++) {
      h = (h + sample(jsonEncode(mediumObj))) & 0x3FFFFFFF;
    }
    return h;
  });
  measure('decode_large', () => valueSum(jsonDecode(large)), rounds: 4);
  measure('encode_large', () => sample(jsonEncode(largeObj)), rounds: 4);
  measure('indented', () {
    var h = 0;
    for (var i = 0; i < 20; i++) {
      h = (h + sample(const JsonEncoder.withIndent('  ').convert(mediumObj))) & 0x3FFFFFFF;
    }
    return h;
  });
  final codec = json.fuse(utf8);
  measure('utf8_bytes', () {
    var h = 0;
    for (var i = 0; i < 20; i++) {
      final b = codec.encode(mediumObj);
      h = (h + b.length + valueSum(codec.decode(b))) & 0x3FFFFFFF;
    }
    return h;
  });
  measure('reviver', () {
    var h = 0;
    for (var i = 0; i < 20; i++) {
      final v = jsonDecode(medium, reviver: (k, v) {
        if (v is double) return v.round();
        if (k == 'name' && v is String) return v.toUpperCase();
        return v;
      });
      h = (h + valueSum(v)) & 0x3FFFFFFF;
    }
    return h;
  });
}
