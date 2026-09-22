// == sobrescrito com hashCode consistente: Set/Map, contains/indexOf, null, identical vs ==.
class Ponto {
  final int x;
  final int y;
  const Ponto(this.x, this.y);

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is Ponto && other.x == x && other.y == y;
  }

  @override
  int get hashCode => Object.hash(x, y);

  @override
  String toString() => 'P($x,$y)';
}

class SemIgualdade {
  final int v;
  SemIgualdade(this.v);
}

class Pessoa {
  final String nome;
  final int idade;
  Pessoa(this.nome, this.idade);

  @override
  bool operator ==(Object other) =>
      other is Pessoa && other.nome.toLowerCase() == nome.toLowerCase();

  @override
  int get hashCode => nome.toLowerCase().hashCode;

  @override
  String toString() => '$nome($idade)';
}

class Contado {
  static int comparacoes = 0;
  final int v;
  Contado(this.v);
  @override
  bool operator ==(Object other) {
    comparacoes++;
    return other is Contado && other.v == v;
  }

  @override
  int get hashCode => v;
}

void main() {
  final a = Ponto(1, 2);
  final b = Ponto(1, 2);
  final c = Ponto(2, 1);
  print(a == b);
  print(a == c);
  print(identical(a, b));
  print(a.hashCode == b.hashCode);
  print(a.hashCode == c.hashCode);
  print(a != c);

  final s = {a, b, c, Ponto(1, 2)};
  print(s.length);
  print(s);
  print(s.contains(Ponto(2, 1)));
  print(s.contains(Ponto(3, 3)));

  final m = <Ponto, String>{};
  m[a] = 'primeiro';
  m[b] = 'segundo';
  m[c] = 'terceiro';
  print(m.length);
  print(m[Ponto(1, 2)]);
  print(m[Ponto(2, 1)]);
  print(m[Ponto(0, 0)]);
  print(m.containsKey(Ponto(1, 2)));

  final lista = [Ponto(5, 5), Ponto(6, 6), Ponto(5, 5)];
  print(lista.contains(Ponto(6, 6)));
  print(lista.indexOf(Ponto(5, 5)));
  print(lista.lastIndexOf(Ponto(5, 5)));
  print(lista.indexOf(Ponto(7, 7)));
  lista.remove(Ponto(5, 5));
  print(lista);
  print(lista.toSet().length);

  Ponto? nulo;
  print(a == nulo);
  print(nulo == a);
  print(nulo == null);
  Object? obj = a;
  print(obj == b);
  print(a == 'texto');
  print(a == Object());

  final s1 = SemIgualdade(1);
  final s2 = SemIgualdade(1);
  print(s1 == s2);
  print(s1 == s1);
  print({s1, s2}.length);

  final p1 = Pessoa('Ana', 30);
  final p2 = Pessoa('ANA', 25);
  print(p1 == p2);
  print({p1, p2}.length);
  print({p1: 1, p2: 2}.length);
  print([p1, Pessoa('Bia', 1)].indexOf(Pessoa('ana', 0)));

  Contado.comparacoes = 0;
  final ct = [Contado(1), Contado(2), Contado(3)];
  print(ct.contains(Contado(3)));
  print(Contado.comparacoes);
  print(ct.contains(Contado(1)));
  print(Contado.comparacoes);
}
