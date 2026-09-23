class A {
//    ^
// [diag.unusedField][column 7][length 0] The value of the field '<unnamed>' isn't used.
  @override
  Object? foo,;
//        ^^^
// [diag.overrideOnNonOverridingField] The field doesn't override an inherited getter or setter.
//            ^
// [diag.missingIdentifier] Expected an identifier.
}
