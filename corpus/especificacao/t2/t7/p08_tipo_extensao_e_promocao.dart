extension type E<X>(int i) {}

void f<T>(E<String> e, T t, T? u) {
  String s1 = e;
  if (t is int) {
    String s2 = t;
    print(s2);
  }
  if (u is int?) {
    String s3 = u;
    print(s3);
  }
  print(s1);
}
