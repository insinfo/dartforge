void f(A x) {
  if (x case R<num> _) {}
}

enum A implements R<int> { v }
class R<T> {}
