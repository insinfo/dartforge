void f(int? x) {
  if (x case > 0) {}
}

extension on int? {
  bool operator >(int other) => true;
}
