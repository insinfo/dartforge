// N13: `for (var i = 0; i < v.length; i++)` sobre uma `List<int>` local, com
// corpo sem chamadas, lê o comprimento e os dados uma vez (voltas
// duplicadas: a lista do runtime pelas voltas rápidas, qualquer outra pelas
// de sempre). Tem de dar o mesmo que a VM em todos os casos.
import 'dart:collection';

/// Uma `List<int>` do programa: o `length` e o `[]` são dela, e o `length`
/// diminui a cada leitura (o laço tem de lê-lo a cada volta).
class Encolhe extends ListBase<int> {
  final List<int> _v;
  var _lidas = 0;
  Encolhe(this._v);
  @override
  int get length {
    final n = _v.length - _lidas;
    if (n > 0) _lidas++;
    return n;
  }

  @override
  set length(int n) => _v.length = n;
  @override
  int operator [](int i) => _v[i];
  @override
  void operator []=(int i, int x) => _v[i] = x;
}

int soma(List<int> v) {
  var s = 0;
  for (var i = 0; i < v.length; i++) {
    s += v[i];
  }
  return s;
}

void dobrar(List<int> v) {
  for (var i = 0; i < v.length; i++) {
    v[i] = v[i] * 2;
  }
}

double media(List<double> v) {
  var s = 0.0;
  for (var i = 0; i < v.length; i++) {
    s += v[i];
  }
  return v.isEmpty ? 0 : s / v.length;
}

int primeiroNegativo(List<int> v) {
  var achado = -1;
  for (var i = 0; i < v.length; i++) {
    if (v[i] >= 0) continue;
    achado = i;
    break;
  }
  return achado;
}

int divideTudo(List<int> v, int d) {
  var s = 0;
  for (var i = 0; i < v.length; i++) {
    s += v[i] ~/ d;
  }
  return s;
}

/// Com chamada no corpo (não é o caminho das voltas rápidas): a chamada
/// cresce a lista, e o laço vê o comprimento novo.
int comChamada(List<int> v) {
  var voltas = 0;
  for (var i = 0; i < v.length; i++) {
    voltas++;
    if (v.length < 6) v.add(i);
  }
  return voltas;
}

void main() {
  final v = [1, 2, 3, 4, 5];
  print(soma(v));
  dobrar(v);
  print(v);
  print(soma([]));
  // Inteiros fora do Smi (guardados em caixa na lista).
  final grandes = [1 << 62, -(1 << 62), 7];
  print(soma(grandes));
  dobrar(grandes);
  print(grandes);
  print(media([1.5, 2.5, 3.0]));
  print(media([]));
  print(primeiroNegativo([3, 1, -4, 1, -5]));
  print(primeiroNegativo([1, 2]));
  // Classe do programa: as voltas de sempre, com o `length` dela a cada volta.
  print(soma(Encolhe([10, 20, 30, 40, 50])));
  final e = Encolhe([1, 2, 3, 4]);
  dobrar(e);
  print(e._v);
  // Uma lista não modificável: a gravação lança como na VM.
  try {
    dobrar(List.unmodifiable([1, 2]));
  } on UnsupportedError catch (err) {
    print('não modificável: ${err.runtimeType}');
  }
  // Exceção no meio das voltas.
  try {
    print(divideTudo([4, 8, 12], 0));
  } on UnsupportedError catch (err) {
    print('divisão: ${err.runtimeType}');
  } on IntegerDivisionByZeroException catch (err) {
    print('divisão: ${err.runtimeType}');
  }
  print(divideTudo([4, 8, 12], 4));
  print(comChamada([0, 0]));
  // Lista de tamanho fixo e a vista de uma sublista.
  final fixa = List<int>.filled(4, 3);
  dobrar(fixa);
  print('${soma(fixa)} $fixa');
  print(soma(v.sublist(1, 3)));
}
