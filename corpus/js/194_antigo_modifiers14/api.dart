base mixin Counter {
  int _count = 1;
  int increment() { _count = _count + 1; return _count; }
  int get current => _count;
}
base class Parent { int prefix = 7; }
interface class Described { String describe() => 'api'; }
sealed class State {}
class Ready extends State {}
class Pending extends State {}
