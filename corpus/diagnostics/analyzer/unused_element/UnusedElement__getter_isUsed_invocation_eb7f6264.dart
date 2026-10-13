abstract class A<T> {
  T get _defaultThing;
  T? _thing;

  void main() {
    _thing ??= _defaultThing;
    print(_thing);
  }
}
class B extends A<int> {
  @override
  int get _defaultThing => 7;
}
