void f() {
  if (0 case _ when 1) {}
//                  ^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
}
