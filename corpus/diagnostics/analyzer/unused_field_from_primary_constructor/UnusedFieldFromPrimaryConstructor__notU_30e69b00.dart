class A({final void _f() = _g}) {}
//                  ^^
// [diag.unusedFieldFromPrimaryConstructor] The value of the field '_f' isn't used.
void _g() {}
