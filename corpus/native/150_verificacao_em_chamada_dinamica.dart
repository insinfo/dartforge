// Chamada dinâmica de closure e de método genérico verifica cada argumento
// contra o tipo do parâmetro, como a entrada dinâmica da VM: tipos nominais,
// `T` da classe, `R` da função externa, `S` do próprio método (com e sem
// argumentos de tipo), tipos de função e anuláveis. Antes, só os parâmetros
// desembalados (`int`, `double`) lançavam.
class Box<T> {
  T? v;
  void put(T x) { v = x; }
  S gen<S>(S x) => x;
  void Function(T) setter() => (T x) { v = x; };
  void each(void Function(T) f) {}
}
void t(String n, void Function() f) {
  try { f(); print('$n: no throw'); } catch (e) { print('$n: ${e.runtimeType}'); }
}
R outer<R>(Object o) {
  dynamic f = (R x) => x;
  return f(o);
}
void genericos() {
  dynamic b = Box<int>();
  t('put', () => b.put('s'));
  t('gen', () => b.gen<int>('s'));
  t('gen ok', () => b.gen<String>('s'));
  t('gen no args', () => b.gen('s'));
  t('gen list', () => b.gen<List<int>>(<String>[]));
  dynamic s = (b as Box<int>).setter();
  t('closure T', () => s('s'));
  t('closure T ok', () => s(1));
  t('outer R', () => outer<int>('s'));
  t('outer R ok', () => outer<String>('s'));
  dynamic fnp = (int Function(int) g) => g(1);
  t('fn param', () => fnp((String x) => 1));
  t('fn param ok', () => fnp((int x) => 1));
  dynamic nul = (int? x) => x;
  t('nullable', () => nul(null));
  dynamic nn = (int x) => x;
  t('null to int', () => nn(null));
}

class A { const A(); }
class B { const B(); }
void literais() {
  dynamic f = (String a1, int a2, A a3, {String n1 = "d", int n2 = -1, A n3 = const A()}) {};
  t('noargs', () => f());
  t('extra named', () => f("x", 1, A(), n4: 4));
  t('pos0', () => f(100, 1, A()));
  t('pos1', () => f("x", 1.1, A()));
  t('pos2', () => f("x", 1, B()));
  t('named1', () => f("x", 1, A(), n1: 100));
  t('named2', () => f("x", 1, A(), n2: "s"));
  t('named3', () => f("x", 1, A(), n3: B()));
  dynamic g = (int x) => x;
  t('g', () => g('s'));
  dynamic h = <T>(T x) => x;
  t('h', () => h<int>('s'));
}

void main() {
  literais();
  genericos();
}
