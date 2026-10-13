typedef Cb = void Function([@Deprecated.optional() int? p]);
//                           ^^^^^^^^^^^^^^^^^^^
// [diag.invalidDeprecatedOptionalAnnotation] The annotation '@Deprecated.optional' can only be applied to optional parameters.
void f(Cb cb) {
  cb();
}
