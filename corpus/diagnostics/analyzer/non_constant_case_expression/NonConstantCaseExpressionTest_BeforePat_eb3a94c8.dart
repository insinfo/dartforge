void f(C e) {
  switch (e) {
    case C.zero:
      break;
    default:
      break;
  }
}

class C {
  static const zero = C(0);

  final int a;
  const C(this.a);
}
