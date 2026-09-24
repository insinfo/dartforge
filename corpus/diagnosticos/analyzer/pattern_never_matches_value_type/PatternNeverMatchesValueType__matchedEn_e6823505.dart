void f(E? x) {
  if (x case E _) {}
}

enum E { v }
