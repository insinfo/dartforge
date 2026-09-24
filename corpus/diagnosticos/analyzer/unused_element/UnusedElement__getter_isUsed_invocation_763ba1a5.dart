class A<T> {
  List<int> _list = List.filled(1, 1);
  int get _item => _list.first;
  set _item(int item) => _list[0] = item;
}
class B<T> {
  A<T> a = A<T>();
}
void main() {
  B<int> b = B();
  b.a._item = 3;
  print(b.a._item == 7);
}
