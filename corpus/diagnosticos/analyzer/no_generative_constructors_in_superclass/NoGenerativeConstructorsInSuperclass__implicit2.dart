class A {
  factory A() => throw '';
}
class B extends A {
//              ^
// [diag.noGenerativeConstructorsInSuperclass] The class 'B' can't extend 'A' because 'A' only has factory constructors (no generative constructors), and 'B' has at least one generative constructor.
}
