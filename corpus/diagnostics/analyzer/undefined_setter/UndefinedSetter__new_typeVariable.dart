f<T>(T t) {
  t.new = 1;
//  ^^^
// [diag.undefinedSetter] The setter 'new' isn't defined for the type 'T'.
}
