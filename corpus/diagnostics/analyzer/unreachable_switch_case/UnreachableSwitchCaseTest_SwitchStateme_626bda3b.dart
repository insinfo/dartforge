enum E { e1, e2 }
void f(E e, bool b) {
  switch (e) {
    case E.e1 when b:
      break;
    case E.e2:
      break;
    case E.e1:
      break;
  }
}
