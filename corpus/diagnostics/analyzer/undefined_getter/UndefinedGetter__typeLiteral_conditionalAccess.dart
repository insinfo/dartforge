class A {}
f() => A?.hashCode;
//      ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
//        ^^^^^^^^
// [diag.undefinedGetter] The getter 'hashCode' isn't defined for the type 'A'.
