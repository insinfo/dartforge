enum E {
  a, b
}

void f(E x) {
  switch (x) {
    case E.a:
    case E.b:
      break;
  }
}
