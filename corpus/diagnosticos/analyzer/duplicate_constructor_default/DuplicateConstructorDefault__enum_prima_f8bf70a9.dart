enum E() {
  v;
  factory E.new() => v;
//        ^^^^^
// [diag.duplicateConstructorDefault] The unnamed constructor is already defined.
}
