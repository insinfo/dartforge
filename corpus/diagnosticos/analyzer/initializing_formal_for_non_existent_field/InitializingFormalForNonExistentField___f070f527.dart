class A {
  int get x => 1;
  A(this.x) {}
//  ^^^^^^
// [diag.initializingFormalForNonExistentField] 'x' isn't a field in the enclosing class.
}
