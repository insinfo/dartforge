class Notifier<T> {
  T _value;

  Notifier(this._value);

  set value(T next) {
    _value = next;
    notify();
  }

  void notify() {
    print('base:$_value');
  }
}

class Derived extends Notifier<String> {
  int holds = 0;

  Derived(String value) : super(value);

  @override
  void notify() {
    if (holds > 0) {
      print('held:$holds');
      return;
    }
    super.notify();
  }
}

class Owner {
  late final Notifier<bool> notifier = Notifier(false);

  void update(bool value) {
    notifier.value = value;
  }
}

void main() {
  final owner = Owner();
  owner.update(true);
  owner.update(false);
  final derived = Derived('first');
  derived.holds = 1;
  derived.value = 'second';
  derived.holds = 0;
  derived.value = 'third';
}
