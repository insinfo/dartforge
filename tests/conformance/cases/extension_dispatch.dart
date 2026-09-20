class Base { int value() { return 2; } }
class Child extends Base { int value() { return 5; } }
extension BaseExtra on Base { int extra() { return this.value() + 1; } }
extension ChildExtra on Child { int extra() { return 99; } }
int callBase(Base value) { return value.extra(); }
void main() {
  print(callBase(Child())); print(Child().extra()); print(Base().extra());
  Base value = Child(); print(value.extra());
  Base? maybe = Child(); print(maybe.extra());
  var inferred = Child(); print(inferred.extra());
}
