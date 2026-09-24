void f() {
  final a = 0;
  @a
//^^
// [diag.invalidAnnotation] Annotation must be either a const variable reference or const constructor invocation.
  var b; // ignore:unused_local_variable
}
