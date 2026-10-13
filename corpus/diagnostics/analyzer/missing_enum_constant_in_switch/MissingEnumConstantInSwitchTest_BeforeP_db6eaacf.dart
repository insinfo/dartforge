enum E { one, two }

void f(E? e) {
  switch (e) {
    case E.one:
      break;
    case E.two:
      break;
    case null:
      break;
  }
}
