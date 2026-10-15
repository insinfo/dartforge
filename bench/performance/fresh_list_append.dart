// Fresh exact user objects appended to a growable typed list. Keep all profiles
// on identical inputs; O4 is a separate unchecked throughput reference.
class Row {
  final int id;
  final int flags;
  Row(this.id, this.flags);
}

List<Row> createRows(int count) {
  final rows = <Row>[];
  for (var i = 0; i < count; i++) {
    rows.add(Row(i, i & 255));
  }
  return rows;
}

int consume(List<Row> rows) => rows.length + rows[0].id + rows.last.id + rows.last.flags;

void main() {
  var checksum = 0;
  for (var i = 0; i < 3; i++) { checksum += consume(createRows(2048)); }
  final watch = Stopwatch()..start();
  for (var i = 0; i < 12; i++) { checksum += consume(createRows(200000)); }
  watch.stop();
  print('checksum=$checksum');
  print('elapsed_us=${watch.elapsedMicroseconds}');
}
