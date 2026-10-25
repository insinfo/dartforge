// Um literal de tipo de classe genérica escrito sem argumentos é o tipo
// instanciado aos limites (`C` é `C<num>` para `class C<T extends num>`); um
// limite em ciclo (`T extends Comparable<T>`) troca os parâmetros do ciclo
// por `dynamic`. Também `toString`/`==` diretos de records nomeados.
class C<T extends num> {}

class D<T extends Comparable<T>> {}

class E<A, B extends A, X extends List<B>> {}

class F<T> {}

class G<T extends Object, U extends Map<String, T>> {}

void main() {
  print([C, D, E, F, G, List, Map]);
  print(identical(C, C<num>));
  print(C == C<int>);
  Type t = G;
  print(t);
  records();
}

// `toString` e `==` chamados diretamente num record com campos nomeados vão
// à forma do record, não ao `_Record` posicional do SDK.
void records() {
  print((1, foo: 2).toString());
  print((foo: 2, 1).toString() == '(1, foo: 2)');
  print((1, foo: 2) == (1, foo: 2));
  print((1, foo: 2).hashCode == (1, foo: 2).hashCode);
}
