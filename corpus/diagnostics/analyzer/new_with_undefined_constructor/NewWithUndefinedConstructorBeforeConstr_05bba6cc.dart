class A {
  A.named() {}
}
f() {
  A.new();
//  ^^^
// [diag.experimentNotEnabled] This requires the 'constructor-tearoffs' language feature to be enabled.
// [diag.newWithUndefinedConstructor] The class 'A' doesn't have a constructor named 'new'.
}
