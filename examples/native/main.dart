int fibonacci(int n) {
  if (n < 2) { return n; }
  return fibonacci(n - 1) + fibonacci(n - 2);
}
void main() {
  print(fibonacci(20));
  var total = 0;
  for (var i = 0; i < 1000; i++) { total += i; }
  print(total);
}
