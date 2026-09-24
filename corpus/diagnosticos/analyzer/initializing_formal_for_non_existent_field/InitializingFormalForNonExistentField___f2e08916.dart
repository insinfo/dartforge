enum E {
  v(0);
  const E(this.x);
//        ^^^^^^
// [diag.initializingFormalForNonExistentField] 'x' isn't a field in the enclosing class.
}
