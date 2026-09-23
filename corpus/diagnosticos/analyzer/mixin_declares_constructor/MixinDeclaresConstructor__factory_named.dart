mixin M {
  factory M.named() => throw 0;
//^^^^^^^
// [diag.mixinDeclaresConstructor] Mixins can't declare constructors.
}
