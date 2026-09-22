// Convertido de tests/native/cases/calls.dart (fixture antigo do backend nativo).
int mark(int x) { print(x); return x; }
int sum(int x, int y) { return x + y; }
bool even(int n) { if (n == 0) { return true; } return odd(n - 1); }
bool odd(int n) { if (n == 0) { return false; } return even(n - 1); }
void done() { print(77); return; }
void main() { print(sum(mark(1), mark(2))); print(even(10)); done(); }
