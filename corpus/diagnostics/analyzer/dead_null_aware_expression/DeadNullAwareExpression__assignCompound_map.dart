class MyMap<K, V> {
  V? operator[](K key) => null;
  void operator[]=(K key, V value) {}
}

f(MyMap<int, int> map) {
  map[0] ??= 0;
}
