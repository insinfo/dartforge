// Convertido de tests/conformance/cases/null_safety.dart (fixture antigo do corpus de conformidade).
int? absent() {}
int fallback() { print('fallback'); return 9; }
int promoted(int? value) {
  if (value == null) { return 0; }
  return value + 1;
}
void main() {
  int? value = absent();
  print(value);
  print(value ?? fallback());
  value = 3;
  print(value ?? fallback());
  print(value! + 2);
  print(promoted(null));
  print(promoted(5));
  String? text = null;
  print(text ?? 'empty');
  bool? flag = false;
  print(flag ?? true);
}
