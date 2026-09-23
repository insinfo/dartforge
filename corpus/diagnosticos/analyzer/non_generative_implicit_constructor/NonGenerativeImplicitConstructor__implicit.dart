class A {
  factory A() => throw 0;
  A.named();
}
class B extends A {
//    ^
// [diag.nonGenerativeImplicitConstructor] The unnamed constructor of superclass 'A' (called by the default constructor of 'B') must be a generative constructor, but factory found.
}
