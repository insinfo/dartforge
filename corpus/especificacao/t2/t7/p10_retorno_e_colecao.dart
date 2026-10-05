typedef Cb = void Function(int);

Cb f(void Function<T>(T) a) {
  return a;
}

List<Cb> g(Map<String, List<int>?> m) {
  return [m];
}
