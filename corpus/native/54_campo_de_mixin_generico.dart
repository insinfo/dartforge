// Inicializador de campo de um mixin genérico aplicado a uma classe cujos
// parâmetros de tipo estão em outra ordem (o `reader` do `FlowModelHelper<Type>`
// do analyzer): a variável de tipo é a do mixin, vista pela aplicação.
mixin Ajudante<T extends Object> {
  final List<T> lista = <T>[];
  List<T> criar() => <T>[];
  bool ehT(Object o) => o is T;
}

class Base<A> {
  final List<A> daBase = <A>[];
}

class Impl<N, S, T extends Object> extends Base<S> with Ajudante<T> {}

void main() {
  final i = Impl<int, String, double>();
  print(i.lista.runtimeType);
  print(i.criar().runtimeType);
  print(i.daBase.runtimeType);
  print([i.ehT(1.5), i.ehT(1)]);
}
