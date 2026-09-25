// %before-language-feature: wildcard-variables

class A {
  void foo(int x) {}
}

class B extends A {
  @override
  void foo(int _) {}
}
