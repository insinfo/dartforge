enum E { e1, e2 }
void f(E e, bool b) {
  switch (e) {
    case E.e1:
      break;
    case E.e2:
      break;
    case E.e1 when b:
//  ^^^^
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
      break;
  }
}
