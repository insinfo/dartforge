class A {
  A.new() {}
//  ^^^
// [diag.experimentNotEnabled] This requires the 'constructor-tearoffs' language feature to be enabled.
}
f() {
  A();
}
