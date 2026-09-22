// Convertido de tests/conformance/cases/loop_evaluation.dart (fixture antigo do corpus de conformidade).
int step(int n) { print(n); return n + 1; }
bool again(int n) { print(n); return n < 3; }
void main() {
  for (var i = 0; i < 3; i = step(i)) { continue; }
  var n = 0;
  do { n++; continue; } while (again(n));
  var product = 1;
  for (var i = 1; i <= 4; i++) { product *= i; }
  print(product);
}
