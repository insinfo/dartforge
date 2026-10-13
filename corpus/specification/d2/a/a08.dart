class G<T> {
  const G();
}
U id<U>(U u) => u;
class K<T> {
  m() {
    const a = G<T>.new;
    const int Function(int) b = id;
    const c = id<T>;
    const T Function(T) d = id;
    const e = 1 is void Function<X>(T, X);
    const f = 1 as T Function();
    return [a, b, c, d, e, f];
  }
}
