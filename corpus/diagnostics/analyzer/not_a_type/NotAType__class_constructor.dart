class A {
  A.foo();
//  ^^^
// [context 1] The declaration of 'foo' is here.
}

A.foo bar() {}
// [diag.notAType][column 1][length 5][context 1] A.foo isn't a type.
