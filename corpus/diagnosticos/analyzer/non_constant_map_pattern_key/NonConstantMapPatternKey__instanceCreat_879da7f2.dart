void f(x) {
  if (x case {A(): 0}) {}
//            ^^^
// [diag.nonConstantMapPatternKey] Key expressions in map patterns must be constants.
}

class A {
  const A();
}
