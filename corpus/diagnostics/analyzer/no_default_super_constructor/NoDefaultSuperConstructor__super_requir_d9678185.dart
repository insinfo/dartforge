class A {
  A({required int? a});
}
class B extends A {}
//    ^
// [diag.noDefaultSuperConstructorImplicit] The superclass 'A' doesn't have a zero argument constructor.
