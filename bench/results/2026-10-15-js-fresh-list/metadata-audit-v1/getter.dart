var reads = 0;
class Row {
  final int id;
  Row(this.id);
  Object? get $ti { reads++; return null; }
}
List<Row> build(int count) {
  final rows = <Row>[];
  for (var i = 0; i < count; i++) { rows.add(Row(i)); }
  return rows;
}
void main() {
  try { print('length=${build(2).length}'); }
  catch (error) { print('typeError=${error is TypeError}'); print('runtime=${error.runtimeType}'); }
  print('reads=$reads');
}
