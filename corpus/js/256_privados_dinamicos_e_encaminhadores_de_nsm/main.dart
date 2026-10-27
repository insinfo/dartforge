// A dynamic access to a private name uses the current library's name. A class
// with noSuchMethod gets forwarders for the private members of its own
// library and throwers for those of another library; a subclass inherits the
// forwarders of a superclass from that library. A private field promoted to a
// callable type is called through its call method, and a private field that
// overrides an inherited private getter is stored apart from it.
import 'other.dart';

class Local {
  int _count = 0;
  int get _getter => 7;
}

class Forwarding implements Local {
  int seen = 0;
  @override
  noSuchMethod(Invocation invocation) => seen += 10;
}

class Remote implements Interface {
  int seen = 0;
  @override
  noSuchMethod(Invocation invocation) => ++seen;
}

class RemoteStubs extends Stubs {}

class Adder {
  int call(int x) => x + 1;
}

class Holder {
  final Object _value;
  Holder(this._value);
  int run() {
    if (_value is Adder) return _value(41);
    return -1;
  }
}

abstract class Base {
  int get _id;
  int describe() => _id;
}

class Derived extends Base {
  @override
  final int _id = 5;
}

void main() {
  dynamic d = Local();
  print(d._count);
  d._count = 3;
  print(d._count);
  print(d._getter);
  var f = Forwarding();
  print(f._count);
  print(f._getter);
  var remote = Remote();
  print(Interface.readHidden(remote));
  print(Interface.readHiddenDynamically(remote));
  print(remote.visible);
  print(remote.seen);
  var stubs = RemoteStubs();
  print(Interface.readHidden(stubs));
  print(stubs.calls);
  print(Holder(Adder()).run());
  print(Derived().describe());
}
