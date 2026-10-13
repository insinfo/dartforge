extension<T> on T Function(T) {
  T Function(T) operator*(T Function(T) other) {
    return (value) => this(other(value));
  }
}

void f() {
  var g = (int i) => i + 1;
  g *= (i) => i + 10;
  print(g(0));
}
