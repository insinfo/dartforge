class M {
  static int Function(int) get f {
    print('getter');
    return (n) => n * 2;
  }

  static int twice(int n) => n * 2;
}

int arg() {
  print('arg');
  return 21;
}

void main() {
  print(M.f(arg()));
  print(M.twice(3));
  final closure = M.f;
  print(closure(4));
}
