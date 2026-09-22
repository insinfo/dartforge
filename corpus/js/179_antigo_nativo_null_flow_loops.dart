// Convertido de tests/native/cases/null_flow_loops.dart (fixture antigo do backend nativo).
int? next(int? n) {
  if (n == null) { return 3; }
  if (n <= 0) { return null; }
  return n - 1;
}
void main() {
  int? n = 3;
  while (n != null) { print(n + 10); n = next(n); }
  print(n);
  bool? flag = true;
  while (flag != null) {
    print(flag);
    if (flag) { flag = false; } else { flag = null; }
  }
  print(flag);
}
