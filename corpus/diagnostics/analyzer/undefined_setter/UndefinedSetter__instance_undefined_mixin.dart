mixin M {
  f() { this.m = 0; }
//           ^
// [diag.undefinedSetter] The setter 'm' isn't defined for the type 'M'.
}
