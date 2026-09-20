int multiply(int a, int b) {
  return a * b;
}
String greet(String name) {
  return 'Olá, ' + name;
}
void announce(int value) {
  return print(value);
}
void main() {
  print(multiply(6, 7));
  print(greet('DartForge'));
  announce(multiply(2, 5));
  print(forward(3));
}
int forward(int value) {
  return value + 1;
}
