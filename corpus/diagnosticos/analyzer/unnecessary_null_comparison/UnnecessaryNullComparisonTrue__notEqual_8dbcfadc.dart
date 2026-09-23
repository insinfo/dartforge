f(Unresolved o) {
//^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
  int? i = o.nullable;
  i != null;
  null != i;
}
