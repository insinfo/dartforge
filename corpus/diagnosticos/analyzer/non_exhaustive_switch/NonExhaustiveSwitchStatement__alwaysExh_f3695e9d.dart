enum E {
  a, b
}

void f(E x) {
  switch (x) {
    case E.a || E.b:
      break;
  }
}
