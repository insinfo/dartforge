// %before-language-feature: super-parameters
class A {
  A(int a);
}
class B extends A {
  B.foo();
//^^^^^
// [diag.noDefaultSuperConstructorExplicit] The superclass 'A' doesn't have a zero argument constructor.
}
