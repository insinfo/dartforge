enum E { one; }
@Deprecated.implement()
// [diag.invalidDeprecatedImplementAnnotation][column 2][length 20] The annotation '@Deprecated.implement' can only be applied to implementable classes.
typedef F = E;
