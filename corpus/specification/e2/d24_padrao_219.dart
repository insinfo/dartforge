// @dart=2.19
void f(Object o) {
  var (a, b) = (1, 2);
  switch (o) { case int x: break; }
  if (o case int y) {}
  var z = switch (o) { _ => 1 };
}
