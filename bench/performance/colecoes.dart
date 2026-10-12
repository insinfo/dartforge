// Coleções do núcleo: List<int>, Map<int, int>, Set<String>.
import 'comum.dart';

int crivo(int n) {
  final primo = List<bool>.filled(n + 1, true);
  var c = 0;
  for (var i = 2; i <= n; i++) {
    if (primo[i]) {
      c++;
      for (var j = i * i; j <= n; j += i) {
        primo[j] = false;
      }
    }
  }
  return c;
}

// `lista_int` em três núcleos: um milhão de `add`, dez milhões de leituras
// indexadas e a ordenação, medidos à parte (cada um paga custos diferentes).
List<int> listaAdd(int n) {
  final l = <int>[];
  for (var i = 0; i < n; i++) {
    l.add(i * 7 % 1000);
  }
  return l;
}

int listaLeitura(List<int> l) {
  var s = 0;
  for (var r = 0; r < 10; r++) {
    for (var i = 0; i < l.length; i++) {
      s += l[i];
    }
  }
  return s;
}

/// Ordena uma cópia; a conferência (ordem e multiconjunto) fica fora da
/// medição, em `conferirOrdenada`.
List<int> ordenar(List<int> origem) => List<int>.of(origem)..sort();

/// Os dados da ordenação: negativos, extremos de 64 bits, a transição entre
/// `int` pequeno e grande (2^62), e trechos já ordenados, reversos e iguais.
List<int> dadosDeOrdenacao(int n) {
  const mn = -9223372036854775808, mx = 9223372036854775807;
  final l = <int>[];
  var x = 88172645463325252;
  for (var i = 0; i < n; i++) {
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    switch (i % 8) {
      case 0:
        l.add(x);
      case 1:
        l.add(x % 1000 - 500);
      case 2:
        l.add((1 << 62) + (x % 3) - 1);
      case 3:
        l.add((i ~/ 8).isEven ? mn : mx);
      case 4:
        l.add(i);
      case 5:
        l.add(n - i);
      default:
        l.add(42);
    }
  }
  return l;
}

void conferirOrdenada(List<int> origem, List<int> ordenada) {
  if (origem.length != ordenada.length) throw StateError('comprimento');
  for (var i = 1; i < ordenada.length; i++) {
    if (ordenada[i - 1] > ordenada[i]) throw StateError('fora de ordem em $i');
  }
  final conta = <int, int>{};
  for (final v in origem) {
    conta[v] = (conta[v] ?? 0) + 1;
  }
  for (final v in ordenada) {
    final c = (conta[v] ?? 0) - 1;
    if (c < 0) throw StateError('elemento $v a mais');
    conta[v] = c;
  }
}

int mapa(int n) {
  final m = <int, int>{};
  for (var i = 0; i < n; i++) {
    m[i * 31 % n] = i;
  }
  var s = 0;
  for (var i = 0; i < n; i++) {
    s += m[i] ?? 0;
  }
  return s;
}

int conjunto(int n) {
  final c = <String>{};
  for (var i = 0; i < n; i++) {
    c.add('k${i % 5000}');
  }
  return c.length;
}

void main() {
  medir('crivo', () => crivo(2000000));
  final adicionada = listaAdd(1000000);
  medir('lista_add', () => listaAdd(1000000).length);
  medir('lista_leitura', () => listaLeitura(adicionada));
  final dados = dadosDeOrdenacao(1000000);
  List<int> ultima = const [];
  medir('lista_sort', () {
    ultima = ordenar(dados);
    return ultima[ultima.length ~/ 2];
  });
  conferirOrdenada(dados, ultima);
  print('ordenacao conferida: ${ultima.first} ${ultima.last}');
  medir('mapa', () => mapa(500000));
  medir('conjunto_str', () => conjunto(300000));
}
