T echo<T>(T value) { return value; }
enum Status {
  ready('ready'), done('done');
  final String text;
  const Status(this.text);
  String get caption => this.text + '!';
}
String show(Status value) => switch (value) { Status.ready => 'R', Status.done => 'D' };
List<List<int>> nestedConstant() { return const <List<int>>[<int>[1, 2]]; }
