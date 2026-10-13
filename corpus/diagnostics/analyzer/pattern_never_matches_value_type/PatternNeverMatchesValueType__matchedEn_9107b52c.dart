void f(A x) {
  if (x case R<num> _) {}
}

enum A<T> implements R<T> {
  v1<String>(),
  v2<int>(),
}

class R<T> {}
