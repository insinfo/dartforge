class A {
  factory A.named() = B;
//                    ^
// [diag.redirectToAbstractClassConstructor] The redirecting constructor 'A.named' can't redirect to a constructor of the abstract class 'B'.
  A();
}

abstract class B extends A {}
