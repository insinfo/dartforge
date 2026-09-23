mixin M {
  var foo = 0;
}

enum E with M {
//          ^
// [diag.enumMixinWithInstanceVariable] Mixins applied to enums can't have instance variables.
  v
}
