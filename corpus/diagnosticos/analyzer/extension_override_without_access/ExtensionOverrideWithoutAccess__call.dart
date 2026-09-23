class C {}

extension E on C {
  int call(int x) => 0;
}

f(C c) {
  E(c)(2);
}
