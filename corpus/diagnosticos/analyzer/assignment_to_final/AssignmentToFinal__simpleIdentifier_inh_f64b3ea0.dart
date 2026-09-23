class A {
  void set foo(int _) {}
}

int get foo => 0;

class B extends A {
  void bar() {
    foo = 0;
//  ^^^
// [diag.assignmentToFinal] 'foo' can't be used as a setter because it's final.
  }
}
