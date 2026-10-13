abstract class A {
  void foo<T extends R, R>();
}

class B implements A {
  void foo<T extends R, R>() {}
}
