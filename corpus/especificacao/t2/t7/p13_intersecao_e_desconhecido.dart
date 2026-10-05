class B<T> {
  B(T t);
}

void g<T extends num>(T t) {
  if (t is int) {
    B<String> b = B(t);
    List<T> l = [t];
    String s = l;
    print([b, l, s]);
  }
}

void h(void Function<T>(List<T> Function<T>(T)) q, Null? n) {
  int x = q;
  int y = n;
  print([x, y]);
}
