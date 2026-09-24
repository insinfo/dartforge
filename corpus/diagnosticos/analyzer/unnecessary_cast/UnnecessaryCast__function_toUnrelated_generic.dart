void f<T extends num>(T Function(T) a) {
  (a as int Function(int))(3);
}
