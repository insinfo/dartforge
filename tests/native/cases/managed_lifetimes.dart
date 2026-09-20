// Fixture original: ordem de efeitos, identidades e vidas úteis entre frames.
class Cell {
  Cell? next = null;
  String text = '';
}
Cell make(String text) {
  Cell result = Cell();
  result.text = text + '!';
  result.next = result;
  return result;
}
String combine(Cell first, Cell second) => first.text + second.text;
void churn() {
  Cell disposable = make('temporary');
  disposable.next = make(disposable.text);
}
String? absent() => null;
Cell? noCell() => null;
void main() {
  Cell retained = make('root');
  for (var i = 0; i < 600; i++) { churn(); }
  print(retained.next == retained);
  print(retained.text);
  print(combine(make('left'), make('right')));
  print(make('same') == make('same'));
  print(absent() == null);
  print(noCell() == null);
  print((noCell() ?? make('fallback')).text);
  String? text = absent();
  print(text == '');
  print(text ?? 'ação 🦀');
}
