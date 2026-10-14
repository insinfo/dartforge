import 'dart:convert';
class OtherList {
  int calls = 0;
  @override
  dynamic noSuchMethod(Invocation invocation) {
    calls++;
    return 7;
  }
}
@pragma('vm:never-inline')
int unknownLength(dynamic value) => value.length;
void main() {
  dynamic list = dartforgeFreshList();
  try { list.add('ssd'); } on UnsupportedError { print('immutable'); }
  print(list);
  print(list.length);
  final other = OtherList();
  print(unknownLength(other));
  print(other.calls);
}
