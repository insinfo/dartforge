var reads = 0;
abstract class Shape {}
class Row implements Shape {
  final int id;
  Row(this.id);
  Object? get $ti { reads++; return null; }
}
List<Shape> build(int count) {
  final rows = <Shape>[];
  for (var i = 0; i < count; i++) { rows.add(Row(i)); }
  return rows;
}
void main() {
  try { print('length=${build(2).length}'); }
  catch (error) { print('typeError=${error is TypeError}'); print('runtime=${error.runtimeType}'); }
  print('reads=$reads');
}
