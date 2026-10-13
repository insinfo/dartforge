mixin M {
  f() { return this.m; }
//                  ^
// [diag.undefinedGetter] The getter 'm' isn't defined for the type 'M'.
}
