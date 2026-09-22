// Sobrecarga de operadores: aritméticos, unário -, [], []=, ==, comparação, bits, compostos.
class Vetor {
  final int x;
  final int y;
  const Vetor(this.x, this.y);

  Vetor operator +(Vetor o) => Vetor(x + o.x, y + o.y);
  Vetor operator -(Vetor o) => Vetor(x - o.x, y - o.y);
  Vetor operator *(int k) => Vetor(x * k, y * k);
  Vetor operator /(int k) => Vetor(x ~/ k, y ~/ k);
  Vetor operator ~/(int k) => Vetor(x ~/ k - 1, y ~/ k - 1);
  Vetor operator %(int k) => Vetor(x % k, y % k);
  Vetor operator -() => Vetor(-x, -y);
  Vetor operator ~() => Vetor(y, x);

  int operator [](int i) => i == 0 ? x : y;

  int get norma1 => x.abs() + y.abs();

  bool operator <(Vetor o) => norma1 < o.norma1;
  bool operator >(Vetor o) => norma1 > o.norma1;
  bool operator <=(Vetor o) => norma1 <= o.norma1;
  bool operator >=(Vetor o) => norma1 >= o.norma1;

  @override
  bool operator ==(Object other) =>
      other is Vetor && other.x == x && other.y == y;

  @override
  int get hashCode => x * 31 + y;

  @override
  String toString() => '($x, $y)';
}

class Matriz {
  final List<List<int>> _dados;
  Matriz(int linhas, int colunas)
      : _dados = List.generate(linhas, (_) => List.filled(colunas, 0));

  List<int> operator [](int i) => _dados[i];
  void operator []=(int i, List<int> linha) => _dados[i] = List.of(linha);

  Matriz operator +(Matriz o) {
    final r = Matriz(_dados.length, _dados[0].length);
    for (var i = 0; i < _dados.length; i++) {
      for (var j = 0; j < _dados[i].length; j++) {
        r[i][j] = _dados[i][j] + o[i][j];
      }
    }
    return r;
  }

  @override
  String toString() => _dados.map((l) => l.join(' ')).join(' | ');
}

class Flags {
  final int bits;
  const Flags(this.bits);
  Flags operator &(Flags o) => Flags(bits & o.bits);
  Flags operator |(Flags o) => Flags(bits | o.bits);
  Flags operator ^(Flags o) => Flags(bits ^ o.bits);
  Flags operator <<(int n) => Flags(bits << n);
  Flags operator >>(int n) => Flags(bits >> n);
  bool tem(Flags o) => (bits & o.bits) == o.bits;
  @override
  String toString() => 'Flags(${bits.toRadixString(2)})';
}

class Celula {
  final Map<String, int> _m = {};
  int operator [](String k) => _m[k] ?? 0;
  void operator []=(String k, int v) {
    _m[k] = v;
  }

  @override
  String toString() => _m.toString();
}

void main() {
  final a = Vetor(1, 2);
  final b = Vetor(3, 5);
  print(a + b);
  print(a - b);
  print(a * 3);
  print(b / 2);
  print(b ~/ 2);
  print(b % 2);
  print(-a);
  print(~a);
  print(a[0]);
  print(a[1]);
  print(a < b);
  print(a > b);
  print(a <= Vetor(2, 1));
  print(a >= b);
  print(a == Vetor(1, 2));
  print(a != b);
  var v = Vetor(0, 0);
  v += a;
  v += a;
  v -= b;
  v *= 2;
  print(v);
  print(-a + b * 2);
  print((a + b) * 2 - a);

  final m = Matriz(2, 2);
  m[0] = [1, 2];
  m[1] = [3, 4];
  m[0][1] = 9;
  print(m);
  print(m[1][0]);
  final soma = m + m;
  print(soma);
  print(soma[1][1]);

  const leitura = Flags(1);
  const escrita = Flags(2);
  const exec = Flags(4);
  final rw = leitura | escrita;
  print(rw);
  print(rw & escrita);
  print(rw ^ escrita);
  print(rw.tem(leitura));
  print(rw.tem(exec));
  print(leitura << 3);
  print(exec >> 2);
  print((rw | exec).bits);

  final c = Celula();
  c['a'] = 1;
  c['b'] = c['a'] + 1;
  c['a'] += 10;
  c['c']++;
  print(c);
  print(c['z']);
  final c2 = Celula()
    ..['x'] = 5
    ..['y'] = 6;
  print(c2);
}
