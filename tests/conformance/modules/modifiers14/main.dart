import 'api.dart';
final class CounterImpl extends Parent with Counter implements Described {
  int _count = 99;
  String describe() => 'counter';
  int ownCount() => this._count;
}
String describeState(State value) => switch(value) {
  Ready() => 'ready',
  Pending() => 'pending',
};
void main() {
  CounterImpl value = CounterImpl();
  print(value.prefix);
  print(value.increment());
  print(value.current);
  print(value.ownCount());
  Described described = value;
  print(described.describe());
  print(describeState(Ready()));
  print(describeState(Pending()));
}
