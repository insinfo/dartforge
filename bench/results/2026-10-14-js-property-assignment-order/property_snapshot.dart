final events = <String>[];

class Cell {
  final String name;
  int _value;
  void Function()? onRead;
  Cell(this.name, this._value);
  int get value { events.add('$name.get'); onRead?.call(); return _value; }
  set value(int next) { events.add('$name.set:$next'); _value = next; }
  int get stored => _value;
}
class OptionalCell {
  final String name;
  int? _value;
  void Function()? onRead;
  OptionalCell(this.name, this._value);
  int? get value { events.add('$name.get'); onRead?.call(); return _value; }
  set value(int? next) { events.add('$name.set:$next'); _value = next; }
  int? get stored => _value;
}
class Holder { final Cell cell; Holder(this.cell); }
extension HolderValue on Holder {
  int get value => cell.value;
  set value(int next) { cell.value = next; }
}
extension type Wrapped(Cell cell) {
  int get value => cell.value;
  set value(int next) { cell.value = next; }
}
extension type OptionalWrapped(OptionalCell cell) {
  int? get value => cell.value;
  set value(int? next) { cell.value = next; }
}
void show(String name) { print('$name: ${events.join(',')}'); events.clear(); }
void main() {
  {
    final first=Cell('plain',1),second=Cell('other',10);
    var target=first;
    int rhs(){events.add('rhs');target=second;return 2;}
    final result=(target.value+=rhs());
    print('plain-prefix: ${first.stored} ${second.stored} $result');show('plain-prefix-order');
  }
  {
    final first=Cell('post',1),second=Cell('other',10);
    var target=first;first.onRead=(){target=second;};
    final result=target.value++;
    print('plain-post: ${first.stored} ${second.stored} $result');show('plain-post-order');
  }
  {
    final first=OptionalCell('null',null),second=OptionalCell('other',10);
    var target=first;first.onRead=(){target=second;};
    int rhs(){events.add('rhs');return 3;}
    final result=(target.value??=rhs());
    print('plain-ifnull: ${first.stored} ${second.stored} $result');show('plain-ifnull-order');
  }
  {
    final first=Cell('extension',1),second=Cell('other',10);
    var target=Holder(first);
    int rhs(){events.add('rhs');target=Holder(second);return 2;}
    target.value+=rhs();
    print('extension-prefix: ${first.stored} ${second.stored}');show('extension-prefix-order');
  }
  {
    final first=Cell('wrapped-post',1),second=Cell('other',10);
    var target=Wrapped(first);first.onRead=(){target=Wrapped(second);};
    final result=target.value++;
    print('wrapped-post: ${first.stored} ${second.stored} $result');show('wrapped-post-order');
  }
  {
    final first=OptionalCell('wrapped-null',null),second=OptionalCell('other',10);
    var target=OptionalWrapped(first);first.onRead=(){target=OptionalWrapped(second);};
    int rhs(){events.add('rhs');return 3;}
    final result=(target.value??=rhs());
    print('wrapped-ifnull: ${first.stored} ${second.stored} $result');show('wrapped-ifnull-order');
  }
}
