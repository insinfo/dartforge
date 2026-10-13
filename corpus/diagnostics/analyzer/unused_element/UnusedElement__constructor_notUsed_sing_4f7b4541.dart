class A {
  A._constructor();
}

class B extends A {
  B() : super._constructor();
  B._named() : super._constructor();
//  ^^^^^^
// [diag.unusedElement] The declaration 'B._named' isn't referenced.
}
