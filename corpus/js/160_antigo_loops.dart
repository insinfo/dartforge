// Convertido de tests/conformance/cases/loops.dart (fixture antigo do corpus de conformidade).
void main() {
  var n = 0;
  var sum = 0;
  while (n < 6) {
    n++;
    if (n == 2) { continue; }
    if (n == 5) { break; }
    sum += n;
  }
  print(sum);
  do {
    n--;
    if (n == 3) { continue; }
    print(n);
  } while (n > 1);
  for (var i = 0; i < 5; ++i) {
    if (i == 2) { continue; }
    sum += i;
  }
  print(sum);
}
