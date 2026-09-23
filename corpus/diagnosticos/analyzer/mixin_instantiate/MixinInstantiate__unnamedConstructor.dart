mixin M {}

void f() {
  new M();
//    ^
// [diag.mixinInstantiate] Mixins can't be instantiated.
}
