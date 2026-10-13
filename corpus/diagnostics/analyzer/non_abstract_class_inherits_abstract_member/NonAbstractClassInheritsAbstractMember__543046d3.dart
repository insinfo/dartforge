mixin M {
  int foo();
}

class A {}
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'M.foo'.

augment class A with M {}
