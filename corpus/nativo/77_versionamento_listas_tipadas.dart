// Versionamento de laços de listas tipadas (`lower/comandos.rs`,
// `laco_de_tipadas`): antes das voltas, a prova de que `i` fica em
// `0..x.length` para toda lista `x[i]`; provada, as voltas sem teste de
// limites (que o LLVM vetoriza); senão, as de sempre. Bordas: a prova que
// falha (limite maior que uma das listas, começo negativo, índice além do
// `Smi`), a visão (outra classe da mesma interface) e a visão não
// modificável na mesma função, visões que se sobrepõem (dependência entre
// voltas), a lista vazia, o estouro de 64 bits e o truncamento na gravação
// (`Int32List`, `Uint8List`, `Int8List`, `Uint16List`, `Float32List`),
// exceção no meio das voltas, `break`/`continue`/`return` e o laço de
// dentro. A saída tem de ser a da VM.
import 'dart:typed_data';

int soma(List<int> v) {
  var h = 0;
  for (final x in v) {
    h = (h * 31 + x) & 0x3fffffff;
  }
  return h;
}

void blend(Int32List src, Int32List dst, Int32List out, int a) {
  final inv = 255 - a;
  for (var i = 0; i < out.length; i++) {
    out[i] = (src[i] * a + dst[i] * inv) >> 8;
  }
}

void copiarMais(Int32List de, Int32List para, int n) {
  for (var i = 0; i < n; i++) {
    para[i] = de[i] + 1;
  }
}

void desdeInicio(Int32List v, int inicio, int fim) {
  for (var i = inicio; i < fim; i++) {
    v[i] = i;
  }
}

void bytes(Uint8List b, Int8List s, Uint16List u, Int32List fonte) {
  for (var i = 0; i < fonte.length; i++) {
    b[i] = fonte[i] * 3;
    s[i] = fonte[i] * 5;
    u[i] = fonte[i] * 7;
  }
}

double escala(Float32List f, Float64List d, double k) {
  var acc = 0.0;
  for (var i = 0; i < d.length; i++) {
    f[i] = d[i] * k;
    acc += f[i];
  }
  return acc;
}

int comSaidas(Int32List v) {
  var n = 0;
  for (var i = 0; i < v.length; i++) {
    if (v[i] < 0) continue;
    if (v[i] > 1000) break;
    if (v[i] == 999) return -i;
    n += v[i];
  }
  return n;
}

int contador = 0;
int tocar(int x) {
  contador++;
  if (x == 13) throw StateError('treze');
  return x * 2;
}

void comChamada(Int32List v, Int32List w) {
  for (var i = 0; i < v.length; i++) {
    w[i] = tocar(v[i]);
  }
}

void passo2(Int64List v) {
  for (var i = 1; i < v.length; i += 2) {
    v[i] = v[i - 1] * 0x100000001;
  }
}

int matriz(Int32List m, int linhas, int colunas) {
  var total = 0;
  for (var y = 0; y < linhas; y++) {
    final linha = Int32List.sublistView(m, y * colunas, (y + 1) * colunas);
    for (var x = 0; x < linha.length; x++) {
      linha[x] = linha[x] * (y + 1);
      total += linha[x];
    }
  }
  return total;
}

void sombra(Int32List v) {
  for (var i = 0; i < v.length; i++) {
    final i2 = v.length - 1 - i;
    v[i2] = v[i2] + i;
    {
      var i = 0;
      v[i] = v[i] + 1;
    }
  }
}

void tentar(String nome, void Function() f) {
  try {
    f();
    print('$nome: ok');
  } catch (e) {
    print('$nome: ${e.runtimeType} $e');
  }
}

void main() {
  const n = 37;
  final src = Int32List(n), dst = Int32List(n), out = Int32List(n);
  for (var i = 0; i < n; i++) {
    src[i] = (i * 97) & 0xff;
    dst[i] = (i * 31 + 7) & 0xff;
  }
  blend(src, dst, out, 173);
  print('blend ${soma(out)}');

  // Estouro: o produto passa de 64 bits (volta) e o resultado não cabe em
  // 32 bits (a gravação trunca).
  final grande = Int32List.fromList([0x7fffffff, -0x80000000, 123456789, -1]);
  final saida = Int32List(4);
  final outra = Int32List.fromList([-7, 0x40000000, 5, -0x80000000]);
  blend(grande, outra, saida, 0x7fffffffffff);
  print('blend grande $saida');
  blend(grande, outra, saida, -0x4000000000000000);
  print('blend negativo $saida');

  // Visões: outra classe da mesma interface, com deslocamento.
  final buf = Int32List(n + 8);
  final visao = buf.buffer.asInt32List(4 * 4, n);
  blend(src, dst, visao, 99);
  print('visão ${soma(visao)} ${buf.sublist(0, 6)}');
  final ro = Int32List(n).asUnmodifiableView();
  tentar('não modificável', () => blend(src, dst, ro, 1));
  blend(src, dst, out, 1);
  print('de novo ${soma(out)}');

  // A prova falha: uma lista menor que o limite (as voltas de sempre, com
  // o RangeError no índice certo e as gravações de antes feitas).
  final curta = Int32List(10);
  final longa = Int32List(n + 3);
  tentar('destino longo', () => blend(src, dst, longa, 7));
  print('longa ${soma(longa)}');
  tentar('fonte curta', () => blend(curta, dst, out, 7));
  print('parcial ${soma(out)}');
  tentar('destino curto', () => copiarMais(src, curta, 12));
  print('curta $curta');
  final v = Int32List(8);
  tentar('negativo', () => desdeInicio(v, -2, 4));
  tentar('além', () => desdeInicio(v, 5, 9));
  print('v $v');
  tentar('além do Smi', () => desdeInicio(v, 0x3fffffffffffffff, 0x4000000000000002));
  tentar('perto do máximo', () => desdeInicio(v, 0x7ffffffffffffff0, 0x7ffffffffffffff2));
  tentar('vazio', () => desdeInicio(v, 3, 3));

  // Sobreposição: a gravação de uma volta é lida na seguinte.
  final sob = Int32List(20);
  for (var i = 0; i < 20; i++) {
    sob[i] = i * 3;
  }
  final atras = sob.buffer.asInt32List(0, 16);
  final frente = sob.buffer.asInt32List(4 * 1, 16);
  copiarMais(atras, frente, 16);
  print('sobreposta $sob');
  copiarMais(sob, sob, sob.length);
  print('a mesma $sob');

  // Truncamento em listas de outros tamanhos.
  final fonte = Int32List.fromList([0, 1, 85, 86, 127, 128, -1, -129, 0x12345678, -0x7fffffff]);
  final b = Uint8List(10), s = Int8List(10), u = Uint16List(10);
  bytes(b, s, u, fonte);
  print('bytes $b $s $u');
  tentar('bytes curto', () => bytes(Uint8List(3), s, u, fonte));
  final vazia = Int32List(0);
  bytes(Uint8List(0), Int8List(0), Uint16List(0), vazia);
  blend(vazia, vazia, vazia, 5);
  print('vazia ok');

  final d = Float64List.fromList([0.1, 1e40, -1e-50, double.nan, double.infinity, 3.4028235e38, 1 / 3]);
  final f = Float32List(7);
  print('escala ${escala(f, d, 3.0)} $f');
  tentar('escala curta', () => escala(Float32List(2), d, 1.0));

  print('saídas ${comSaidas(Int32List.fromList([1, -5, 2, 3, 2000, 7]))}');
  print('saídas ${comSaidas(Int32List.fromList([4, 999, 1]))}');
  print('saídas ${comSaidas(vazia)}');

  final w = Int32List(6);
  tentar('chamada', () => comChamada(Int32List.fromList([1, 2, 3, 13, 5, 6]), w));
  print('chamada $w $contador');

  final l64 = Int64List(9);
  for (var i = 0; i < 9; i += 2) {
    l64[i] = 0x7fffffff + i;
  }
  passo2(l64);
  print('passo 2 $l64');

  final m = Int32List(12);
  for (var i = 0; i < 12; i++) {
    m[i] = i + 1;
  }
  print('matriz ${matriz(m, 3, 4)} $m');
  final sv = Int32List.fromList([10, 20, 30, 40]);
  sombra(sv);
  print('sombra $sv');
}
