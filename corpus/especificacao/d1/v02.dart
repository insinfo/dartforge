extension type const E(int it) {}
const E0 = E(0);
const e1 = const E(1);
class A {
  A({E a = const E(0)});
}
extension type const Bool(bool _) implements bool {
  static const Bool isTrue = Bool(true);
}
void f(int x) {
  switch (x) {
    case == E0:
  }
}
