// Fixture original: referências locais sobrevivem à reutilização de temporários.
class Cell {
  int number = 0;
  String text = '';
  Cell? next = null;
}
Cell make(int number) {
  Cell cell = Cell();
  cell.number = number;
  cell.text = 'a' + 'b';
  cell.next = cell;
  return cell;
}
int combine(Cell first, Cell second) => first.number + second.number;
void main() {
  Cell saved = make(42);
  Cell current = saved;
  Cell? previous = null;
  int checksum = 0;
  for (var i = 0; i < 1000; i++) {
    previous = current;
    current = make(i);
    checksum = checksum + combine(make(1), make(2));
    if (i == 0) { continue; }
    if (previous != null) { checksum = checksum + previous.number; }
    {
      Cell current = make(99);
      if (current.number != 99) { return; }
    }
  }
  print(saved.number);
  print(saved.next == saved);
  print(saved.text);
  print(current.number);
  print(previous!.number);
  print(checksum);
}
