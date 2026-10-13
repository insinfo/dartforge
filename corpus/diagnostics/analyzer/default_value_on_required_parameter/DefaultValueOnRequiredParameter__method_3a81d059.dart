class C {
  void foo({required int? a = 0}) {}
//                        ^
// [diag.defaultValueOnRequiredParameter] Required named parameters can't have a default value.
}
