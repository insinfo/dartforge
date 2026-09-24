void f(List<Object> a) {
  var [dynamic? _] = a;
//            ^
// [diag.unnecessaryQuestionMark] The '?' is unnecessary because 'dynamic' is nullable without it.
}
