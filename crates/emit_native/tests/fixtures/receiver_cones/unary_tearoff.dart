class Vector {
  final int x;
  const Vector(this.x);
  Vector operator -() => Vector(-x);
  int read() => x;
}

class UnusedUnary {
  final int x;
  const UnusedUnary(this.x);
  UnusedUnary operator -() => UnusedUnary(-x);
}

void main(List<String> arguments) {
  final vector = Vector(7);
  final negated = -vector;
  final read = negated.read;
  print(read());
  print((-13).abs());
  print(UnusedUnary(19).x);
  if (arguments.contains('dynamic')) {
    final dynamic value = UnusedUnary(23);
    final UnusedUnary result = -value;
    print(result.x);
  }
}
