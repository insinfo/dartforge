void f<T>(A<T> x) {
  if (x case R<int> _) {}
}

final class A<T> extends R<T> {}
class R<T> {}
