// Programa original: valores salvos devem sobreviver ao reuso de slots do laco.
class Box {
  String text = '';
  Box? next = null;
}
Box make(int i) {
  Box value = Box();
  if (i == 0) { value.text = 'first' + '!'; }
  else { value.text = 'next' + '!'; }
  value.next = value;
  return value;
}
String joined(String first, String second) { return first + second; }
String recursive(int n, String saved) {
  if (n == 0) { return saved; }
  String next = 'step' + '!';
  return joined(saved, recursive(n - 1, next));
}
void run(int count) {
  Box? saved = null;
  String kept = '';
  for (var i = 0; i < count; i++) {
    Box current = make(i);
    if (i == 0) { saved = current; kept = current.text; }
    current = make(i + 1);
    if (i == 2) { continue; }
    if (i == count - 1) { print(current.text); }
  }
  print(kept);
  print(saved!.text);
  print(saved!.next == saved);
}
void main() {
  run(8);
  run(2000);
  print(recursive(3, 'start!'));
}
