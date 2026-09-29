// Listas tipadas pelo cabeçalho de endereço fixo (`heap::CabecalhoTipado`,
// `lower/tipados.rs`): o comprimento e os dados lidos em linha na lista
// interna, e a visão (sem cabeçalho próprio) pelo caminho do runtime. Todos
// os tipos de elemento, extremos, visões com deslocamento, a não
// modificável, a vazia, o índice fora da faixa, a `Uint8ClampedList`,
// SIMD, cópias e o coletor com muitas listas.
import 'dart:isolate';
import 'dart:typed_data';

void tente(String rotulo, void Function() f) {
  try {
    f();
  } catch (e) {
    print('$rotulo: ${e.runtimeType} $e');
  }
}

int somaU8(Uint8List l) {
  var s = 0;
  for (var i = 0; i < l.length; i++) {
    s += l[i];
  }
  return s;
}

void copia(Uint8List destino, List<int> origem, int de) {
  for (var i = 0; i < origem.length; i++) {
    destino[de + i] = origem[i];
  }
}

class Construtor {
  Uint8List _buffer = Uint8List(4);
  int _n = 0;
  void add(List<int> bytes) {
    final precisa = _n + bytes.length;
    if (_buffer.length < precisa) {
      final novo = Uint8List(precisa * 2);
      novo.setRange(0, _n, _buffer);
      _buffer = novo;
    }
    for (var i = 0; i < bytes.length; i++) {
      _buffer[_n + i] = bytes[i];
    }
    _n = precisa;
  }

  Uint8List pegar() => Uint8List.view(_buffer.buffer, 0, _n);
}

void main() async {
  final u8 = Uint8List(5);
  for (var i = 0; i < u8.length; i++) {
    u8[i] = i * 100;
  }
  print('u8 $u8 ${u8.length} soma ${somaU8(u8)}');
  final i8 = Int8List.fromList([127, -128, 255, 256, -1]);
  i8[0] = i8[0] + 1;
  print('i8 $i8');
  final u16 = Uint16List(3)..[0] = 65535..[1] = 65536..[2] = -1;
  final i16 = Int16List(3)..[0] = 32767..[1] = 32768..[2] = -32769;
  print('16 $u16 $i16');
  final u32 = Uint32List(2)..[0] = 0xFFFFFFFF..[1] = -1;
  final i32 = Int32List(2)..[0] = 0x7FFFFFFF..[1] = 0x80000000;
  print('32 $u32 $i32');
  final i64 = Int64List(2)..[0] = 0x7FFFFFFFFFFFFFFF..[1] = -0x8000000000000000;
  final u64 = Uint64List(1)..[0] = -1;
  print('64 $i64 $u64');
  final f32 = Float32List(3)..[0] = 1.1..[1] = 1e40..[2] = -0.0;
  final f64 = Float64List(3)..[0] = 1.1..[1] = double.nan..[2] = double.infinity;
  print('f $f32 $f64 ${f32[2].isNegative}');
  final cl = Uint8ClampedList(3)..[0] = 300..[1] = -5..[2] = 128;
  print('clamped $cl');

  // Vazia e fora da faixa.
  final vazia = Uint8List(0);
  print('vazia ${vazia.length} ${somaU8(vazia)}');
  tente('vazia[0]', () => vazia[0]);
  tente('u8[5]', () => u8[5]);
  tente('u8[-1]', () => u8[-1]);
  tente('u8[5]=', () => u8[5] = 1);
  tente('f64[3]', () => f64[3]);

  // Visões: com deslocamento, sobre outro tipo, não modificável.
  final base = Uint8List.fromList(List.generate(16, (i) => i));
  final visao = Uint8List.view(base.buffer, 4, 8);
  print('visao $visao ${visao.length} soma ${somaU8(visao)}');
  visao[0] = 200;
  print('base ${base[4]}');
  tente('visao[8]', () => visao[8]);
  final v32 = Uint32List.view(base.buffer, 8, 2);
  v32[1] = 0x01020304;
  print('v32 $v32 ${base.sublist(12)}');
  final sub = Uint8List.sublistView(base, 2, 6);
  print('sublistView $sub');
  final imut = base.asUnmodifiableView();
  print('imut ${imut[4]} ${imut.length}');
  tente('imut[0]=', () => imut[0] = 1);
  final imutF = f64.asUnmodifiableView();
  print('imutF ${imutF[0]}');
  tente('imutF[0]=', () => imutF[0] = 2.0);

  // `List<int>` estático sobre a lista tipada e sobre outra lista.
  List<int> li = u8;
  print('como List ${li[1]} ${li.length}');
  li[1] = 7;
  print('gravada ${u8[1]}');
  tente('List<int> fora', () => li[99]);

  // Copia byte a byte de uma lista de código (o `writeHeaders`).
  final destino = Uint8List(10);
  copia(destino, 'Host:'.codeUnits, 0);
  copia(destino, [1, 2, 3], 5);
  print('copia $destino');
  final c = Construtor();
  for (var k = 0; k < 20; k++) {
    c.add('ab$k'.codeUnits);
  }
  final pronto = c.pegar();
  print('construtor ${pronto.length} ${String.fromCharCodes(pronto.sublist(0, 12))}');

  // SIMD.
  final f4 = Float32x4List(2);
  f4[0] = Float32x4(1, 2, 3, 4);
  f4[1] = f4[0] * f4[0];
  print('simd ${f4[1].w} ${f4.length}');
  tente('simd fora', () => f4[2]);
  final i4 = Int32x4List.view(Uint8List(32).buffer, 16, 1);
  i4[0] = Int32x4(1, 2, 3, 4);
  print('simd visao ${i4[0].z}');

  // Muitas listas (o coletor), e cópia entre isolados.
  var total = 0;
  for (var k = 0; k < 2000; k++) {
    final l = Uint16List(k % 50 + 1);
    for (var i = 0; i < l.length; i++) {
      l[i] = i + k;
    }
    total += l[l.length - 1];
  }
  print('muitas $total');
  final outra = await Isolate.run(() {
    final l = Float64List(4);
    for (var i = 0; i < 4; i++) {
      l[i] = i / 2;
    }
    return l;
  });
  print('isolado $outra ${outra[3]}');
  final bd = ByteData(8)..setInt32(0, -2)..setFloat32(4, 1.5);
  final comoU8 = bd.buffer.asUint8List();
  print('bytedata $comoU8');
}
