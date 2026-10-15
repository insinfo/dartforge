abstract class Shape {}
class Row implements Shape {
  final int $ti;
  Row(this.$ti);
}
List<Shape> build(int count) {
  final rows = <Shape>[];
  for (var i = 0; i < count; i++) { rows.add(Row(i)); }
  return rows;
}
void main() {
  try { print('length=${build(2).length}'); }
  catch (error) { print('typeError=${error is TypeError}'); print('runtime=${error.runtimeType}'); }
}
