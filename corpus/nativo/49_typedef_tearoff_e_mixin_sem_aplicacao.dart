// Tear-off de construtor por um typedef de classe genérica
// (`providerFactory: NotifierFamilyProvider.internal` do riverpod, com
// `typedef NotifierFamilyProvider<…> = FamilyNotifierProviderImpl<…>`), com
// os argumentos de tipo inferidos pelo contexto ou genérico; e um mixin com
// campo que nenhuma classe aplica (o `LocatorMixin` do `state_notifier`): o
// acesso ao campo não executa, e o programa compila.
class Impl<N, T> {
  final String nome;
  Impl.interno(this.nome);
  Impl(this.nome);
  @override
  String toString() => 'Impl<$N, $T>($nome)';
}

typedef Alias<N, T> = Impl<N, T>;
typedef Fixo<T> = Impl<int, T>;

class Familia<N, T> {
  final Impl<N, T> Function(String) fabrica;
  Familia({required this.fabrica});
}

class Concreta extends Familia<String, double> {
  Concreta() : super(fabrica: Alias.interno);
}

mixin Localizador {
  Object? _local;
  Object? get local => _local;
  set local(Object? v) => _local = v;
}

void main() {
  print(Concreta().fabrica('a'));
  Impl<bool, int> Function(String) f = Alias.new;
  print(f('b'));
  Impl<int, String> Function(String) g = Fixo.interno;
  print(g('c'));
  final h = Alias.interno;
  print(h<num, Object>('d'));
  print(Localizador);
}
