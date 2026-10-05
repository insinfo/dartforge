int top = 0;
class K { int fld = 0; void m() { (fld) = 1; } }
void f(Object o, int par) {
  (x) = 0;
  [y, par] = [1, 2];
  (top) = 0;
  switch (o) {
    case foo:
      break;
    case == unresolved:
      break;
    case const (bar):
      break;
  }
  if (o case zed) {}
  if (o case == b && var b) {}
}
