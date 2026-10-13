void f<T, U extends num, V extends List<V>, W extends U>(T t, U u, V v, T? tq, U? uq, W w, Object? x) {
  if (t case 1) {}
  if (u case 1) {}
  if (u case 'a') {}
  if (u case null) {}
  if (uq case null) {}
  if (uq case 'a') {}
  if (v case 1) {}
  if (w case 'a') {}
  if (tq case null) {}
  if (t is int) {
    if (t case true) {}
    if (t case (true)) {}
    if (t case 1) {}
  }
  if (x case T) {}
  if (x case const (List<T>)) {}
  if (x case const (T)) {}
  if (x case List<T>) {}
  if (x case const <T>[]) {}
  if (x case == T) {}
}
