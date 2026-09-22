// `super == other` e `super.hashCode` dentro de `operator ==`/`hashCode` sobrescritos.
class Base {
  final int a;
  Base(this.a);
  @override
  bool operator ==(Object other) => other is Base && other.a == a;
  @override
  int get hashCode => a;
}

class Derivada extends Base {
  final String b;
  Derivada(int a, this.b) : super(a);
  @override
  bool operator ==(Object other) {
    if (other is! Derivada) return super == other;
    return super == other && b == other.b;
  }
  @override
  int get hashCode => super.hashCode ^ b.hashCode;
}

class SemEquals {
  @override
  bool operator ==(Object other) => super == other;
  @override
  int get hashCode => super.hashCode + 1;
}

void main() {
  final d1 = Derivada(1, 'x');
  final d2 = Derivada(1, 'x');
  final d3 = Derivada(1, 'y');
  print(d1 == d2);
  print(d1 == d3);
  print(d1 == Base(1));
  print(d1 == Base(2));
  print(d1.hashCode == d2.hashCode);
  final s = SemEquals();
  print(s == s);
  print(s == SemEquals());
  print(s.hashCode == s.hashCode);
  print(s.hashCode != SemEquals().hashCode);
}
