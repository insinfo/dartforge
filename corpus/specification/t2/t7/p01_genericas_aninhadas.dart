class A<T> {
  void Function<S>(S, T) get g => throw 0;
}

void f<S>(A<S> a, void Function<T>(T, void Function<T>(T)) h,
    T Function<T extends num>(T) k) {
  int x = a.g;
  int y = h;
  int z = k;
  print([x, y, z]);
}
