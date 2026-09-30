// Listas tipadas no espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §2.5,
// §2.14, §5.3): todos os tipos (Int8…Float64, Uint8Clamped, Int32x4, Float32x4,
// Float64x2), visões e visões não modificáveis (`asUnmodifiableView`; as classes
// `Unmodifiable*View` saíram do SDK), `ByteData` com endian, `sublistView`,
// listas maiores que 16 KiB, SIMD em lista, e `asTypedList` sobre memória de
// `malloc` obtido por `DynamicLibrary.process()` (como 08_ffi e 12_ffi_structs).
import 'dart:ffi';
import 'dart:typed_data';

typedef MallocC = Pointer<Void> Function(IntPtr);
typedef MallocD = Pointer<Void> Function(int);
typedef FreeC = Void Function(Pointer<Void>);
typedef FreeD = void Function(Pointer<Void>);

void tentar(String nome, Object? Function() f) {
  try {
    print('$nome: ${f()}');
  } on UnsupportedError catch (e) {
    print('$nome: UnsupportedError ${e.message}');
  } on RangeError catch (e) {
    print('$nome: RangeError ${e.message}');
  } catch (e) {
    print('$nome: ${e.runtimeType} $e');
  }
}

int resumo(List<num> l) {
  var h = 0;
  for (final x in l) {
    h = (h * 31 + x.hashCode) & 0x3FFFFFFF;
  }
  return h;
}

int resumoBytes(ByteBuffer b, [int offset = 0, int? len]) {
  final v = b.asUint8List(offset, len);
  var h = 0;
  for (final x in v) {
    h = (h * 31 + x) & 0x3FFFFFFF;
  }
  return h;
}

void main() {
  // Todos os tipos, com estouro e arredondamento.
  const valores = [0, 1, -1, 127, 128, 255, 256, 32767, 32768, 65535, 65536, -129, 2147483647, 2147483648, -2147483649];
  final i8 = Int8List.fromList(valores);
  final u8 = Uint8List.fromList(valores);
  final u8c = Uint8ClampedList.fromList(valores);
  final i16 = Int16List.fromList(valores);
  final u16 = Uint16List.fromList(valores);
  final i32 = Int32List.fromList(valores);
  final u32 = Uint32List.fromList(valores);
  final i64 = Int64List.fromList([...valores, 9223372036854775807, -9223372036854775807 - 1]);
  final u64 = Uint64List.fromList([...valores, -1]);
  print('Int8: $i8');
  print('Uint8: $u8');
  print('Uint8Clamped: $u8c');
  print('Int16: $i16');
  print('Uint16: $u16');
  print('Int32: $i32');
  print('Uint32: $u32');
  print('Int64: $i64');
  print('Uint64: $u64');
  final f32 = Float32List.fromList([0.1, 1e40, -1e-50, 3.14159265358979, double.nan, -0.0]);
  final f64 = Float64List.fromList([0.1, 1e300, -1e-320, 3.14159265358979, double.infinity, -0.0]);
  print('Float32: $f32');
  print('Float64: $f64');
  for (final l in <TypedData>[i8, u8, u8c, i16, u16, i32, u32, i64, u64, f32, f64]) {
    print('  ${l.runtimeType}: elementSize=${l.elementSizeInBytes} lengthInBytes=${l.lengthInBytes}');
  }
  u8c[0] = 300;
  u8c[1] = -5;
  i8[0] = 200;
  u16[0] = -2;
  print('gravações com estouro: ${u8c[0]} ${u8c[1]} ${i8[0]} ${u16[0]}');

  // SIMD.
  final f4 = Float32x4List(4);
  for (var i = 0; i < 4; i++) {
    f4[i] = Float32x4(i + 0.5, i * 2.0, -i.toDouble(), 1.0);
  }
  final i4 = Int32x4List.fromList([Int32x4(1, -2, 3, -4), Int32x4.bool(true, false, true, false)]);
  final d2 = Float64x2List(3);
  d2[0] = Float64x2(1.5, -2.5);
  d2[1] = d2[0] * Float64x2.splat(2.0);
  d2[2] = d2[1].abs();
  var acc = Float32x4.zero();
  for (final v in f4) {
    acc = acc + v;
  }
  print('Float32x4 soma: ${acc.x} ${acc.y} ${acc.z} ${acc.w}');
  print('Float32x4List: ${f4.length} ${f4.lengthInBytes} ${f4[3].x} ${f4[3].shuffle(Float32x4.wzyx).x}');
  print('Int32x4List: ${i4[0].x} ${i4[0].y} ${i4[1].flagX} ${i4[1].flagY} ${i4[1].signMask}');
  print('Float64x2List: ${d2[1].x} ${d2[1].y} ${d2[2].y} ${d2.lengthInBytes}');
  final vistaF = f4.buffer.asFloat32List();
  print('Float32x4 como Float32List: ${vistaF.length} ${vistaF.sublist(4, 8)}');
  final sel = i4[1].select(Float32x4(1, 2, 3, 4), Float32x4(-1, -2, -3, -4));
  print('select: ${sel.x} ${sel.y} ${sel.z} ${sel.w}');
  final vistaI4 = Int32x4List.sublistView(Int32List.fromList([1, 2, 3, 4, 5, 6, 7, 8]), 4);
  print('Int32x4 sublistView: ${vistaI4.length} ${vistaI4[0].w}');

  // ByteData com endian.
  final bd = ByteData(32);
  bd.setUint32(0, 0x01020304);
  bd.setUint32(4, 0x01020304, Endian.little);
  bd.setInt16(8, -2, Endian.big);
  bd.setFloat64(16, 1.5, Endian.little);
  bd.setFloat32(24, -0.25);
  bd.setInt64(10, -9223372036854775807 - 1, Endian.little);
  print('ByteData: ${bd.buffer.asUint8List()}');
  print('leituras: ${bd.getUint32(0, Endian.little).toRadixString(16)} ${bd.getUint32(4).toRadixString(16)} '
      '${bd.getInt8(3)} ${bd.getUint16(0)} ${bd.getFloat64(16, Endian.little)} ${bd.getFloat32(24)} '
      '${bd.getInt64(10, Endian.little)} ${bd.getUint64(10, Endian.big).toRadixString(16)}');
  tentar('ByteData fora', () => bd.getInt32(30));
  final bdVista = ByteData.sublistView(bd, 16, 24);
  print('ByteData sublistView: ${bdVista.lengthInBytes} ${bdVista.offsetInBytes} ${bdVista.getFloat64(0, Endian.little)}');

  // Visões sobre o mesmo buffer.
  final base = Uint8List(64);
  for (var i = 0; i < 64; i++) {
    base[i] = i;
  }
  final v16 = base.buffer.asUint16List(8, 4);
  final v32 = Int32List.view(base.buffer, 16, 2);
  final v64 = Float64List.sublistView(base, 32, 48);
  print('visões: $v16 $v32 ${v64.length} ${v64.offsetInBytes}');
  v16[0] = 0xFFFF;
  v32[1] = -1;
  print('gravado pela visão: ${base.sublist(8, 12)} ${base.sublist(20, 24)}');
  final sv = Uint8List.sublistView(base, 60);
  sv[0] = 99;
  print('sublistView: $sv ${base[60]} offset=${sv.offsetInBytes}');
  tentar('visão desalinhada', () => base.buffer.asUint32List(2));
  tentar('sublistView fora', () => Uint8List.sublistView(base, 10, 100));
  tentar('Int32List.sublistView desalinhada', () => Int32List.sublistView(base, 3, 8));
  final copia = base.sublist(0, 4);
  copia[0] = 77;
  print('sublist é cópia: ${copia.runtimeType} ${base[0]}');

  // Visões não modificáveis.
  final naoMod = base.asUnmodifiableView();
  print('não modificável: ${naoMod.offsetInBytes} ${naoMod.length} ${naoMod[5]}');
  tentar('grava na não modificável', () => naoMod[0] = 1);
  tentar('setRange na não modificável', () => naoMod.setRange(0, 2, [1, 2]));
  base[5] = 55;
  print('não modificável acompanha: ${naoMod[5]}');
  final naoModBuffer = naoMod.buffer;
  tentar('buffer da não modificável', () {
    naoModBuffer.asUint8List()[0] = 3;
    return base[0];
  });
  for (final t in <TypedData>[
    i8.asUnmodifiableView(),
    i16.asUnmodifiableView(),
    f64.asUnmodifiableView(),
    f4.asUnmodifiableView(),
    i4.asUnmodifiableView(),
    d2.asUnmodifiableView(),
    bd.asUnmodifiableView(),
  ]) {
    tentar('grava em visão de ${t.elementSizeInBytes} bytes (${t.lengthInBytes})', () {
      if (t is ByteData) {
        t.setInt8(0, 1);
      } else if (t is Float32x4List) {
        t[0] = Float32x4.zero();
      } else if (t is Int32x4List) {
        t[0] = Int32x4(0, 0, 0, 0);
      } else if (t is Float64x2List) {
        t[0] = Float64x2.zero();
      } else if (t is List<int>) {
        (t as List<int>)[0] = 1;
      } else if (t is List<double>) {
        (t as List<double>)[0] = 1.0;
      }
      return 'gravou';
    });
  }

  // Listas maiores que 16 KiB.
  final g8 = Uint8List(100000);
  for (var i = 0; i < g8.length; i++) {
    g8[i] = (i * 31) & 0xFF;
  }
  final g64 = Float64List(40000);
  for (var i = 0; i < g64.length; i++) {
    g64[i] = i / 7;
  }
  final g32 = Int32List(50000)..fillRange(0, 50000, -3);
  g32.setRange(100, 200, List.generate(100, (i) => i));
  print('grande Uint8: ${g8.lengthInBytes} ${resumoBytes(g8.buffer)}');
  print('grande Float64: ${g64.lengthInBytes} ${g64[39999]} ${resumo(g64)}');
  print('grande Int32: ${g32.lengthInBytes} ${g32[150]} ${g32[49999]} ${resumo(g32)}');
  final gsub = g8.sublist(50000, 50010);
  print('grande sublist: $gsub');
  final gview = Uint16List.sublistView(g8, 20000, 20020);
  print('grande visão: $gview');
  g64.sort((a, b) => b.compareTo(a));
  print('grande sort: ${g64.first} ${g64.last}');
  final lista = List<Uint8List>.generate(20, (i) => Uint8List(20000)..[i] = i);
  var somaL = 0;
  for (final l in lista) {
    somaL += l.fold<int>(0, (a, b) => a + b);
  }
  print('várias grandes: $somaL');

  // asTypedList sobre memória nativa.
  final lib = DynamicLibrary.process();
  final malloc = lib.lookupFunction<MallocC, MallocD>('malloc');
  final free = lib.lookupFunction<FreeC, FreeD>('free');
  final p = malloc(4096).cast<Uint8>();
  final ext = p.asTypedList(4096);
  for (var i = 0; i < ext.length; i++) {
    ext[i] = i & 0xFF;
  }
  print('externa: ${ext.runtimeType} ${ext.length} ${ext[255]} ${ext[256]} ${resumoBytes(ext.buffer, ext.offsetInBytes, 4096)}');
  print('ponteiro vê a gravação: ${p[4095]} ${(p + 100).value}');
  p[10] = 250;
  print('lista vê o ponteiro: ${ext[10]}');
  final ext32 = p.cast<Int32>().asTypedList(1024);
  ext32[0] = -1;
  print('Int32 externa: ${ext32[0]} ${ext[0]} ${ext[3]} ${ext32[1].toRadixString(16)}');
  final extF = p.cast<Double>().asTypedList(512);
  extF[1] = 2.75;
  print('Float64 externa: ${extF[1]} ${ext.sublist(8, 16)}');
  final extVista = Uint8List.sublistView(ext, 1000, 1010);
  print('visão da externa: $extVista ${extVista.offsetInBytes}');
  final extBd = ByteData.sublistView(ext);
  extBd.setUint16(2000, 0xABCD, Endian.big);
  print('ByteData na externa: ${ext[2000].toRadixString(16)} ${ext[2001].toRadixString(16)}');
  final extCopia = Uint8List.fromList(ext);
  final extNaoMod = ext.asUnmodifiableView();
  tentar('externa não modificável', () => extNaoMod[0] = 1);
  free(p.cast());
  print('cópia sobrevive ao free: ${extCopia.length} ${extCopia[255]} ${extCopia[10]}');

  // SIMD sobre memória nativa.
  final ps = malloc(64).cast<Float>();
  final nativoF = ps.asTypedList(16);
  for (var i = 0; i < 16; i++) {
    nativoF[i] = i * 0.5;
  }
  final simd = Float32x4List.view(nativoF.buffer, nativoF.offsetInBytes, 4);
  var t = Float32x4.zero();
  for (final v in simd) {
    t += v;
  }
  print('SIMD nativo: ${t.x} ${t.y} ${t.z} ${t.w}');
  free(ps.cast());
}
