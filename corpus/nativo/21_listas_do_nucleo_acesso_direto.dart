// O acesso direto às listas do núcleo com o tipo estático `List<E>`
// (`lower/tipados.rs`): `[]`, `[]=`, os compostos e `length` das listas do
// runtime; uma classe do usuário que implementa `List`, as listas não
// modificáveis, a covariância e os índices fora dos limites ficam com o
// despacho, com os erros da VM.
import 'dart:collection';

class Contada extends ListBase<int> {
  final List<int> _dados;
  int leituras = 0;
  Contada(int n) : _dados = List<int>.filled(n, 0);
  @override
  int get length => _dados.length;
  @override
  set length(int n) => _dados.length = n;
  @override
  int operator [](int i) {
    leituras++;
    return _dados[i] * 10;
  }

  @override
  void operator []=(int i, int v) => _dados[i] = v + 1;
}

int soma(List<int> a) {
  var s = 0;
  for (var i = 0; i < a.length; i++) {
    s += a[i];
  }
  return s;
}

double somaD(List<double> a) {
  var s = 0.0;
  for (var i = 0; i < a.length; i++) {
    s += a[i];
  }
  return s;
}

void erro(String nome, void Function() f) {
  try {
    f();
    print('$nome: sem erro');
  } on RangeError catch (e) {
    print('$nome: RangeError ${e.start} ${e.end} ${e.invalidValue}');
  } on UnsupportedError catch (e) {
    print('$nome: UnsupportedError ${e.message}');
  } on TypeError catch (_) {
    print('$nome: TypeError');
  }
}

void main() {
  final fixa = List<int>.filled(5, 2);
  final crescente = <int>[1, 2, 3];
  final grande = <int>[9007199254740993, -9223372036854775808];
  print('soma: ${soma(fixa)} ${soma(crescente)} ${grande[0]} ${grande[1]}');

  // Compostos e escrita.
  fixa[0] += 10;
  fixa[1]++;
  fixa[2] = fixa[2] * fixa[3];
  final antes = fixa[4]--;
  print('fixa: $fixa $antes');

  // Crescer dentro do laço: o comprimento é relido a cada volta.
  final c = <int>[0];
  for (var i = 0; i < c.length; i++) {
    if (c.length < 6) c.add(c[i] + i + 1);
  }
  print('crescente: $c');
  c.removeLast();
  c.length = 3;
  print('encolhida: $c ${c.length}');

  // double e bool sem caixa.
  final d = <double>[0.5, 1.25, -3.0];
  d[1] *= 2;
  d.add(0.1);
  print('double: ${somaD(d)} $d');
  final b = List<bool>.filled(3, false);
  b[1] = true;
  b[2] = !b[1];
  print('bool: $b ${b[1]}');

  // Elementos de referência e anuláveis.
  final s = <String>['a', 'b'];
  s[0] = s[0] + s[1];
  final n = <int?>[1, null, 3];
  n[1] = n[0]! + n[2]!;
  final o = <Object>[1, 'x', 2.5, true];
  print('ref: $s $n ${o[0]} ${o[1]} ${o[2]} ${o[3]}');

  // Classe do usuário: o despacho, não o runtime.
  final u = Contada(3);
  u[0] = 4;
  u[1] += 2;
  print('usuário: ${u[0]} ${u[1]} ${u.length} ${soma(u)} leituras=${u.leituras}');

  // Não modificáveis e const.
  const k = <int>[7, 8, 9];
  print('const: ${k[2]} ${k.length} ${soma(k)}');
  erro('escrita em const', () => k[0] = 1);
  final inm = List<int>.unmodifiable([1, 2]);
  erro('composto em não modificável', () => inm[0] += 1);
  final visao = UnmodifiableListView<int>(crescente);
  erro('escrita na visão', () => visao[0] = 5);
  print('visão: ${visao[1]} ${visao.length}');

  // Covariância: `List<num>` que é `List<int>`.
  List<num> nums = <int>[1, 2, 3];
  nums[0] = 5;
  erro('double em List<int>', () => nums[1] = 2.5);
  print('covariância: $nums');

  // Fora dos limites.
  erro('ler -1', () => fixa[-1]);
  erro('ler length', () => crescente[crescente.length]);
  erro('gravar fora', () => fixa[5] = 0);
  erro('composto fora', () => d[9] += 1);
  final vazia = <int>[];
  erro('vazia', () => vazia[0]);
  print('vazia: ${vazia.length} ${soma(vazia)}');
}
