mixin M {
  factory M() => throw 0;
//^^^^^^^
// [diag.mixinDeclaresConstructor] Mixins can't declare constructors.
}
