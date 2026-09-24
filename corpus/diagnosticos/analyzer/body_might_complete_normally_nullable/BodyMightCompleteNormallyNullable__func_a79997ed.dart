enum E { a, b }

extension type EE(E it) {}

int f(EE e) {
  switch (e) {
    case E.a:
      return 0;
    case E.b:
      return 1;
  }
}
