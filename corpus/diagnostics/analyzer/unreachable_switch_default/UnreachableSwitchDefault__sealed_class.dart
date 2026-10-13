sealed class A {}
class B extends A {}
class C extends A {}

String f(A x) {
  switch (x) {
    case B():
      return 'B';
    case C():
      return 'C';
    default:
//  ^^^^^^^
// [diag.unreachableSwitchDefault] This default clause is covered by the previous cases.
      return 'Some other subclass of A (impossible)';
  }
}
