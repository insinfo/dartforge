void zeroDivisor(int n) {
  for (var i = 0; i < n; i++) { print(i % 0); }
}
void main() {
  try { zeroDivisor(1); } catch (error) { print('zeroError=${error is UnsupportedError}'); }
}
