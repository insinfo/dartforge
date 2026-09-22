// Convertido de tests/conformance/cases/merge_bindings.dart (fixture antigo do corpus de conformidade).
int soma(int n1, int n2) => n1 + n2;
int soma2(int left, int right) => left + right;
int difference(int left, int right) => left - right;
int reverse(int left, int right) => right - left;
int nested(int value) {
  int result = value;
  { int value = 7; result = result + value; }
  return result;
}
int nested2(int input) {
  int output = input;
  { int input = 7; output = output + input; }
  return output;
}
void main() {
  print(soma(20, 22));
  print(soma2(3, 4));
  print(difference(10, 3));
  print(reverse(10, 3));
  print(nested(2));
  print(nested2(3));
}
