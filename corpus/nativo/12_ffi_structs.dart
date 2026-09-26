// dart:ffi estágio 2: layout de Struct/Union (alinhamento, @Packed, @Array
// multidimensional, structs aninhadas e arrays de structs), acesso a campos
// por Pointer.ref e índice, cópia de struct por atribuição, asTypedList
// sobre memória nativa e a extensão `call` de um Allocator.
import 'dart:ffi';

final class Ponto extends Struct {
  @Int32()
  external int x;
  @Int32()
  external int y;
  @Double()
  external double peso;
  external Pointer<Int8> nome;
}

final class Caixa extends Struct {
  external Ponto a;
  external Ponto b;
  @Array(4)
  external Array<Uint8> marcas;
  @Uint8()
  external int flag;
}

final class Par extends Struct {
  @Int16()
  external int a;
  @Bool()
  external bool ok;
}

final class Grade extends Struct {
  @Array(2, 3)
  external Array<Array<Int32>> m;
  @Array(3)
  external Array<Par> pares;
  @Long()
  external int longo;
  @Size()
  external int tamanho;
  external Pointer<Grade> proxima;
}

final class Pacote extends Union {
  @Int32()
  external int i;
  @Float()
  external double f;
}

@Packed(1)
final class Compacta extends Struct {
  @Uint8()
  external int a;
  @Uint32()
  external int b;
}

typedef MallocC = Pointer<Void> Function(IntPtr);
typedef MallocD = Pointer<Void> Function(int);
typedef CallocC = Pointer<Void> Function(IntPtr, IntPtr);
typedef CallocD = Pointer<Void> Function(int, int);
typedef FreeC = Void Function(Pointer<Void>);
typedef FreeD = void Function(Pointer<Void>);

final _lib = DynamicLibrary.process();
final _malloc = _lib.lookupFunction<MallocC, MallocD>('malloc');
final _calloc = _lib.lookupFunction<CallocC, CallocD>('calloc');
final _free = _lib.lookupFunction<FreeC, FreeD>('free');

/// Um alocador zerado, como o `calloc` do package:ffi.
class Zerado implements Allocator {
  const Zerado();
  @override
  Pointer<T> allocate<T extends NativeType>(int bytes, {int? alignment}) => _calloc(bytes, 1).cast();
  @override
  void free(Pointer<NativeType> p) => _free(p.cast());
}

const zerado = Zerado();

void main() {
  print([sizeOf<Ponto>(), sizeOf<Caixa>(), sizeOf<Pacote>(), sizeOf<Compacta>(), sizeOf<Par>(), sizeOf<Grade>()]);

  final p = _malloc(sizeOf<Caixa>() * 2).cast<Caixa>();
  final c = p.ref;
  c.a.x = 10;
  c.a.y = -20;
  c.a.peso = 1.5;
  c.b.x = 30;
  c.marcas[0] = 7;
  c.marcas[3] = 9;
  c.flag = 255;
  print([c.a.x, c.a.y, c.a.peso, c.b.x, c.marcas[0], c.marcas[3], c.flag]);
  p[1].a.x = 99;
  print([p[1].a.x, (p + 1).ref.a.x]);
  final pp = p.cast<Ponto>();
  print([pp.ref.x, pp.ref.y]);
  p[1].b = c.a;
  print([p[1].b.x, p[1].b.y, p[1].b.peso]);

  final u = _malloc(8).cast<Pacote>();
  u.ref.f = 1.0;
  print(u.ref.i);

  final g = zerado<Grade>(2);
  final r = g.ref;
  for (var i = 0; i < 2; i++) {
    for (var j = 0; j < 3; j++) {
      r.m[i][j] = i * 10 + j;
    }
  }
  print([r.m[0][0], r.m[0][2], r.m[1][1], r.m[1][2]]);
  r.pares[1].a = -7;
  r.pares[1].ok = true;
  r.pares[2].a = 300;
  print([r.pares[0].a, r.pares[1].a, r.pares[1].ok, r.pares[2].a, r.pares[0].ok]);
  r.longo = -123456789012;
  r.tamanho = 42;
  print([r.longo, r.tamanho]);
  r.proxima = g + 1;
  r.proxima.ref.longo = 77;
  print([g[1].longo, r.proxima.address - g.address]);

  final ints = zerado<Int32>(4);
  ints.asTypedList(4).setAll(0, [1, 2, 3, 4]);
  print(ints[2]);
  final lista = ints.asTypedList(4);
  lista[3] = 99;
  print([ints[3], lista]);
  final bytes = ints.cast<Uint8>().asTypedList(16);
  print(bytes.sublist(0, 8));

  zerado.free(ints);
  zerado.free(g);
  _free(u.cast());
  _free(p.cast());
}
