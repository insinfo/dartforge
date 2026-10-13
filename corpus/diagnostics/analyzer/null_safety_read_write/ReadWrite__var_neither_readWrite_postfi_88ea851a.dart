void f(bool b) {
  // ignore:unused_local_variable
  var x;
  if (b) x = 0;
  ++x; // 0
}
