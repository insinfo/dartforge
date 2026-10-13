class A {}

enum E {
  v.foo();
  const A.foo();
//      ^
// [diag.invalidConstructorName] The name of a constructor must match the name of the enclosing class.
  const E.foo();
//        ^^^
// [diag.unusedElement] The declaration 'E.foo' isn't referenced.
}
