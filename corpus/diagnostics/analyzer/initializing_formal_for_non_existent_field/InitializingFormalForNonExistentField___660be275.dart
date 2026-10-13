class A {
  int x = 1;
}
class B extends A {
  B(this.x) {}
//  ^^^^^^
// [diag.initializingFormalForNonExistentField] 'x' isn't a field in the enclosing class.
}
