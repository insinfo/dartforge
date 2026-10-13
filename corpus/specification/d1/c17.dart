T id<T>(T x) => x;
const int Function(int) a = id;
const b = id<int>;
class C<T> {
  const C();
  void m<U>() {
    const c = C<T>();
    const d = id<U>;
    const T Function(T) e = id;
    const f = T;
    const g = <T>[];
  }
}
const h = id<int, int>;
