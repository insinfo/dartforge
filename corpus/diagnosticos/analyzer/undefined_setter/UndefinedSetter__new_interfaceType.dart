class C {}

f() {
  C.new = 1;
//  ^^^
// [diag.undefinedSetter] The setter 'new' isn't defined for the type 'C'.
}
