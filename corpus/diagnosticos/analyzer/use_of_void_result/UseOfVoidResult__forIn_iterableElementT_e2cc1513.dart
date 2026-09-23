void f(List<void> values) {
  Object? object;
  dynamic anything;
  for (object in values) {
//               ^^^^^^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
    object;
  }
  for (anything in values) {
//                 ^^^^^^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
    anything;
  }
}
