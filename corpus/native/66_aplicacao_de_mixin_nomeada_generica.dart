// `A<T>.nome(…)` de uma aplicação de mixin nomeada (`class A<T> = B<T> with
// M<T>;`): o construtor encaminhado da superclasse, com o objeto `A<T>` e os
// argumentos de tipo escritos (ou vindos de uma variável de tipo). Sem o
// caminho próprio em `lower/chamadas.rs` a chamada terminava em exceção não
// tratada no backend nativo.
class B<T> {
  final T v;
  B.nome(this.v);
  B(this.v);
}

mixin M<T> on B<T> {
  String m() => 'M $v ${v.runtimeType}';
}

class A<T> = B<T> with M<T>;

class C<T> {
  A<T> fazer(T x) => A<T>.nome(x);
}

A<X> f<X>(X x) => A<X>.nome(x);

void main() {
  final a = A<int>.nome(3);
  print(a.runtimeType);
  print(a.m());
  print(a is A<int>);
  print(a is A<String>);
  print(a is B<int>);
  print(a is M<int>);
  final b = A<String>('x');
  print(b.runtimeType);
  final c = A.nome(2.5);
  print(c.runtimeType);
  print(f<num>(1).runtimeType);
  print(f('s').runtimeType);
  print(C<List<int>>().fazer([1]).runtimeType);
  final lista = <B<Object>>[A<int>.nome(1), A<String>.nome('z')];
  for (final e in lista) {
    print('${e.runtimeType} ${e.v}');
  }
}
