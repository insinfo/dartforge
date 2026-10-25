// Um literal de tipo de classe genérica escrito sem argumentos é o tipo
// instanciado aos limites (`C` é `C<num>` para `class C<T extends num>`); um
// limite em ciclo (`T extends Comparable<T>`) troca os parâmetros do ciclo
// por `dynamic`.
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
}
