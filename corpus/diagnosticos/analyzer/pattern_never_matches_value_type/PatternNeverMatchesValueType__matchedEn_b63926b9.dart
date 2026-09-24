void f<T>(E<T>? x) {
  if (x case E<num> _) {}
}

enum E<T> { v<int>() }
