// An old-style generic function parameter (`bar<V>(V v)`) keeps its type
// parameters: it can be called with explicit type arguments and a generic
// function passed to it is not instantiated.
class C<T> {
  String bar<V>(T t, int u, V v) => '$t$u$v<$V>';
  String foo<U>(String bar<V>(T t, U u, V v)) => bar<int>(1 as T, 2 as U, 3);
}

int test(int f(int x, int y)) => f(123, 456);

int run(T minFn<T extends num>(T x, T y)) => test(minFn);

T smaller<T extends num>(T x, T y) => x < y ? x : y;

void main() {
  var c = C<int>();
  print(c.foo<int>(c.bar));
  print(run(smaller));
}
