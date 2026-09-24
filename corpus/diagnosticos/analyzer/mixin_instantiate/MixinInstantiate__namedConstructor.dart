mixin M {
  M.named() {}
//^
// [diag.mixinDeclaresConstructor] Mixins can't declare constructors.
}

void f() {
  new M.named();
//    ^
// [diag.mixinInstantiate] Mixins can't be instantiated.
}
