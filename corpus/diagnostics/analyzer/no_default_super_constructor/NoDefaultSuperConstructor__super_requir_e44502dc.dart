class A {
  A(int p);
}
class B extends A {}
//    ^
// [diag.noDefaultSuperConstructorImplicit] The superclass 'A' doesn't have a zero argument constructor.
