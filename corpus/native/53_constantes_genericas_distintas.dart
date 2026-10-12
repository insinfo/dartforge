// Constantes de classe genérica com argumentos de tipo diferentes são objetos
// diferentes (`const Link<Token?>()` e `const Link<BeginToken>()` do parser e
// do scanner do analyzer): a canonização conta o tipo da criação.
class Elo<T> {
  const Elo();
  Elo<T> antes(T elemento) => Entrada<T>(elemento, this);
}

class Entrada<T> extends Elo<T> {
  final T cabeca;
  final Elo<T> cauda;
  Entrada(this.cabeca, this.cauda);
}

class Par<A, B> {
  final A a;
  const Par(this.a);
}

void main() {
  const x = Elo<int>();
  const y = Elo<String?>();
  print(x.runtimeType);
  print(y.runtimeType);
  print(identical(x, y));
  print(identical(const Elo<int>(), x));
  var z = y;
  z = z.antes(null);
  print(z.runtimeType);
  print(const Par<int, String>(1).runtimeType);
  print(const Par<int, bool>(1).runtimeType);
  print(identical(const Par<int, String>(1), const Par<int, bool>(1)));
}
