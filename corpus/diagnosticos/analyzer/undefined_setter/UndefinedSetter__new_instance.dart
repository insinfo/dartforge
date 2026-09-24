class C {}

f(C c) {
  c.new = 1;
//  ^^^
// [diag.undefinedSetter] The setter 'new' isn't defined for the type 'C'.
}
