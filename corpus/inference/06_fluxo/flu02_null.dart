// R-FLU-02: `== null` / `!= null` (dos dois lados) promovem para NonNull.
void f(int? x, int? y, int? z) {
  if (x != null) print(/*@*/x);
  if (x == null) return;
  print(/*@*/x);
  if (null != y) {
    print(/*@*/y);
  } else {
    print(/*@*/y);
  }
  if (z == null) {
    print(/*@*/z);
  }
}

void main() => f(1, 2, 3);
