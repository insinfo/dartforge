extension type E(int it) {
  E.named(this.it, super.foo);
//                 ^^^^^
// [diag.extensionTypeConstructorWithSuperFormalParameter] Extension type constructors can't declare super formal parameters.
}
