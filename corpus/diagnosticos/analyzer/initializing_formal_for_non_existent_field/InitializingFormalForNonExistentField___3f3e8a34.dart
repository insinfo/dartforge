extension type E(int it) {
  E.named(this.x) : this.it = 0;
//        ^^^^^^
// [diag.initializingFormalForNonExistentField] 'x' isn't a field in the enclosing class.
}
