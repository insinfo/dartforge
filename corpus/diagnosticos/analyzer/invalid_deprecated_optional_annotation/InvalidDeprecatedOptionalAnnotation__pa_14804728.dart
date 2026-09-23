void f(void Function([@Deprecated.optional() int? p]) cb) {
//                     ^^^^^^^^^^^^^^^^^^^
// [diag.invalidDeprecatedOptionalAnnotation] The annotation '@Deprecated.optional' can only be applied to optional parameters.
  cb();
}
