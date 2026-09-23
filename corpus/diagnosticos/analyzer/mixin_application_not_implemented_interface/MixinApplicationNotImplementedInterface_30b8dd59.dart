abstract class A {}

mixin M on A {}

enum E with M {
//          ^
// [diag.mixinApplicationNotImplementedInterface] 'M' can't be mixed onto 'Enum' because 'Enum' doesn't implement 'A'.
  v
}
