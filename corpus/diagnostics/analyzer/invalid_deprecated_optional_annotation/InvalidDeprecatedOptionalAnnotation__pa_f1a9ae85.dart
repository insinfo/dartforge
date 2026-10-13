void f(
  @Deprecated.optional() int? p
// ^^^^^^^^^^^^^^^^^^^
// [diag.invalidDeprecatedOptionalAnnotation] The annotation '@Deprecated.optional' can only be applied to optional parameters.
) {}
