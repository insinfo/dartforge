// `for-in` sobre `List<E>` pelo índice (`lower/sdk_fonte.rs`,
// `for_in_de_lista`): mesma ordem, mesmos elementos e o mesmo
// `ConcurrentModificationError` do `ListIterator`; uma classe do usuário que
// implementa `List` segue pelo `Iterator` dela.
import 'dart:collection';

class MinhaLista extends ListBase<int> {
  final _dados = <int>[10, 20, 30];
  int iteradores = 0;
  @override
  int get length => _dados.length;
  @override
  set length(int n) => _dados.length = n;
  @override
  int operator [](int i) => _dados[i] * 2;
  @override
  void operator []=(int i, int v) => _dados[i] = v;
  @override
  Iterator<int> get iterator {
    iteradores++;
    return super.iterator;
  }
}

Future<int> somaAssincrona(List<int> l) async {
  var s = 0;
  for (final x in l) {
    await Future<void>.delayed(Duration.zero);
    s += x;
  }
  return s;
}

void main() async {
  final vazia = <int>[];
  for (final x in vazia) {
    print('nunca $x');
  }
  final fixa = List<int>.filled(4, 7);
  var s = 0;
  for (final x in fixa) {
    s += x;
    fixa[0] = 100; // gravar elemento não é modificação estrutural
  }
  print('fixa $s ${fixa[0]}');
  final nomes = ['a', 'b', 'c', 'd', 'e'];
  final saida = <String>[];
  fora:
  for (final n in nomes) {
    for (final m in nomes) {
      if (m == 'c') continue;
      if (m == 'e') continue fora;
      if (n == 'd') break fora;
      saida.add('$n$m');
    }
  }
  print(saida.join(','));
  final crescendo = <int>[1, 2, 3];
  try {
    for (final x in crescendo) {
      if (x == 2) crescendo.add(99);
    }
  } on ConcurrentModificationError catch (e) {
    print('modificada: ${identical(e.modifiedObject, crescendo)} ${crescendo.length}');
  }
  final encolhendo = <int>[1, 2, 3];
  try {
    for (final x in encolhendo) {
      if (x == 1) encolhendo.removeLast();
    }
  } on ConcurrentModificationError {
    print('encolheu');
  }
  final minha = MinhaLista();
  var t = 0;
  for (final x in minha) {
    t += x;
  }
  print('minha $t ${minha.iteradores}');
  final List<Object?> mista = [1, 2.5, 'x', null, true, [1], 1 << 62];
  for (final o in mista) {
    print('${o.runtimeType} $o');
  }
  final ds = <double>[1.5, 2.5];
  var d = 0.0;
  for (var x in ds) {
    x *= 2;
    d += x;
  }
  print('ds $d $ds');
  var ultimo = -1;
  for (ultimo in [4, 5, 6]) {}
  print('ultimo $ultimo');
  final closures = <int Function()>[];
  for (final x in [1, 2, 3]) {
    closures.add(() => x * 10);
  }
  print(closures.map((f) => f()).toList());
  print('async ${await somaAssincrona([1, 2, 3, 4])}');
  final List<int> imutavel = List.unmodifiable([3, 4]);
  for (final x in imutavel) {
    print('imutavel $x');
  }
  final grande = List<int>.generate(100000, (i) => i);
  var g = 0;
  for (final x in grande) {
    g += x;
  }
  print('grande $g');
}
