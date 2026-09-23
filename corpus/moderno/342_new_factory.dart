// requer-dart: 3.13
// `new`/`factory` sem o nome da classe (3.13), corpo `;`, e `factory()` que
// passa a ser construtor.
class Q {
  int a, b;
  new(this.a, this.b);
  new origem()
      : a = 0,
        b = 0;
  new redir() : this.origem();
  factory clone(Q o) => Q(o.a, o.b);
  const factory constante() = QC;
  String get s => '$a,$b';
}

class QC implements Q {
  const QC();
  @override
  int get a => 1;
  @override
  set a(int v) {}
  @override
  int get b => 2;
  @override
  set b(int v) {}
  @override
  String get s => 'QC';
}

class Vazia;

class Fabrica {
  final int n;
  new _(this.n);
  factory() => Fabrica._(7);
}

class Generica<T>(final T valor) {
  new vazia(T v) : this(v);
}

void main() {
  print(Q(1, 2).s);
  print(Q.origem().s);
  print(Q.redir().s);
  print(Q.clone(Q(3, 4)).s);
  print(Fabrica().n);
  print(const Q.constante().s);
  print(Vazia().runtimeType);
  print(Generica<int>(5).valor);
  print(Generica.vazia('x').valor);
}
