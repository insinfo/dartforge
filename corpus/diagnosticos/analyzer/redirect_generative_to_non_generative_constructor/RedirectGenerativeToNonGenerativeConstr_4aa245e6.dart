class A {
  A() : this.x();
//      ^^^^^^^^
// [diag.redirectGenerativeToNonGenerativeConstructor] Generative constructors can't redirect to a factory constructor.
  factory A.x() => throw 0;
}
