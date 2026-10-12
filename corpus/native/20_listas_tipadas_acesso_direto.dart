// O acesso direto às listas tipadas numéricas (`lower/tipados.rs`): `[]`,
// `[]=`, os compostos e `length` com o tipo estático de lista tipada leem e
// gravam os elementos sem despacho; os casos que o caminho rápido recusa
// (índice fora dos limites, visão não modificável, `Uint8ClampedList`) caem
// no `typed_data_patch.dart`, com os erros da VM.
import 'dart:typed_data';

int somaInt32(Int32List a) {
  var s = 0;
  for (var i = 0; i < a.length; i++) {
    s += a[i];
  }
  return s;
}

double somaFloat32(Float32List a) {
  var s = 0.0;
  for (var i = 0; i < a.length; i++) {
    s += a[i];
  }
  return s;
}

void preencher(Uint8List a, int k) {
  for (var i = 0; i < a.length; i++) {
    a[i] = i * 37 + k;
  }
}

void erro(String nome, void Function() f) {
  try {
    f();
    print('$nome: sem erro');
  } on RangeError catch (e) {
    print('$nome: RangeError ${e.start} ${e.end} ${e.invalidValue}');
  } on UnsupportedError catch (e) {
    print('$nome: UnsupportedError ${e.message}');
  }
}

void main() {
  // Truncamento e extensão de sinal de cada largura.
  final i8 = Int8List(4);
  i8[0] = 127;
  i8[1] = 128;
  i8[2] = -129;
  i8[3] = 0x1ff;
  print('Int8List: $i8');
  final u8 = Uint8List(3);
  u8[0] = -1;
  u8[1] = 256 + 7;
  u8[2] = 255;
  print('Uint8List: $u8');
  final i16 = Int16List(2)
    ..[0] = 0x18000
    ..[1] = -32769;
  print('Int16List: $i16');
  final u16 = Uint16List(2)
    ..[0] = -2
    ..[1] = 0x12345;
  print('Uint16List: $u16');
  final i32 = Int32List(3);
  i32[0] = 0x80000000;
  i32[1] = -1;
  i32[2] = 0x7fffffff;
  print('Int32List: $i32');
  final u32 = Uint32List(2);
  u32[0] = -1;
  u32[1] = 0x100000005;
  print('Uint32List: $u32');
  final i64 = Int64List(2);
  i64[0] = -9223372036854775808;
  i64[1] = 9223372036854775807;
  print('Int64List: $i64');
  final u64 = Uint64List(1);
  u64[0] = -1;
  print('Uint64List: ${u64[0]} ${u64.length}');

  // Ponto flutuante: `Float32List` arredonda para precisão simples.
  final f32 = Float32List(3);
  f32[0] = 0.1;
  f32[1] = 1e40;
  f32[2] = -0.0;
  print('Float32List: ${f32[0]} ${f32[1]} ${f32[2]} ${f32[2].isNegative}');
  final f64 = Float64List(2);
  f64[0] = 0.1;
  f64[1] = double.nan;
  print('Float64List: ${f64[0]} ${f64[1].isNaN}');

  // Compostos: `+=`, `++`, `??=` não se aplica (elemento não anulável).
  final c = Int32List(3);
  c[0] += 5;
  c[1]++;
  c[2] -= 3;
  c[0] *= 3;
  final antes = c[1]++;
  print('compostos: $c $antes');
  final d = Float64List(2);
  d[0] += 1.5;
  d[1] -= 0.25;
  d[0] *= d[0];
  print('compostos double: $d');

  // Laços.
  final grande = Int32List(1000);
  for (var i = 0; i < grande.length; i++) {
    grande[i] = i - 500;
  }
  print('soma Int32: ${somaInt32(grande)}');
  final fl = Float32List(100);
  for (var i = 0; i < fl.length; i++) {
    fl[i] = i * 0.5;
  }
  print('soma Float32: ${somaFloat32(fl)}');
  final bytes = Uint8List(10);
  preencher(bytes, 3);
  print('preencher: $bytes');

  // Visões: deslocamento, comprimento e escrita através da visão.
  final base = Uint8List.fromList(List.generate(16, (i) => i));
  final visao = Uint8List.sublistView(base, 4, 10);
  print('visão: ${visao.length} ${visao[0]} ${visao[5]}');
  visao[1] = 99;
  print('base após escrita na visão: ${base[5]}');
  final como32 = Int32List.view(base.buffer, 4, 2);
  como32[0] = -2;
  print('Int32 sobre bytes: ${como32.length} ${como32[0]} ${base.sublist(4, 8)}');
  final visao32 = Int32List.sublistView(grande, 10, 20);
  print('visão Int32: ${visao32.length} ${visao32[0]} ${somaInt32(visao32)}');

  // Não modificável: leitura direta, escrita recusada como na VM.
  final ro = grande.asUnmodifiableView();
  print('não modificável: ${ro.length} ${ro[3]} ${somaInt32(ro)}');
  erro('escrita na visão não modificável', () => ro[0] = 1);
  erro('composto na visão não modificável', () => ro[0] += 1);
  print('intacta: ${grande[0]}');

  // Clamped: satura (fica no `[]=` do SDK).
  final cl = Uint8ClampedList(3);
  cl[0] = 300;
  cl[1] = -5;
  cl[2] = 128;
  print('Uint8ClampedList: $cl ${cl[0]}');

  // Fora dos limites: os erros da VM.
  erro('ler -1', () => i32[-1]);
  erro('ler length', () => i32[i32.length]);
  erro('gravar 3', () => i32[3] = 1);
  erro('gravar -1 Float32', () => f32[-1] = 1.0);
  erro('composto fora', () => c[10] += 1);
  erro('visão fora', () => visao[6]);
  final vazia = Float64List(0);
  erro('vazia', () => vazia[0]);
  print('vazia: ${vazia.length}');
}
