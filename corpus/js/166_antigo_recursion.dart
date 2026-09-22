// Convertido de tests/conformance/cases/recursion.dart (fixture antigo do corpus de conformidade).
int factorial(int n) {
  if (n <= 1) {
    return 1;
  } else {
    return n * factorial(n - 1);
  }
}
bool positive(int n) {
  if (n > 0) { return true; }
  return false;
}
void main() {
  print(factorial(6));
  print(positive(4));
  print(positive(-2));
}
