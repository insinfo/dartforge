enum E { e1, e2 }

String f(E e) {
  switch (e) {
    case E.e1:
      return 'e1';
    case E.e2:
      return 'e2';
    default:
//  ^^^^^^^
// [diag.unreachableSwitchDefault] This default clause is covered by the previous cases.
      return 'Some other value of E (impossible)';
  }
}
