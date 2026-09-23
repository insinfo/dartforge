enum E {
  one, two;

  static const x = 0;
}

void f(E e) {
  switch (e) {
    case E.one:
      break;
    case E.two:
      break;
  }
}
