extension type E(int i) {
  E._named(this.i);
//  ^^^^^^
// [diag.unusedElement] The declaration 'E._named' isn't referenced.
}
typedef A = E;
