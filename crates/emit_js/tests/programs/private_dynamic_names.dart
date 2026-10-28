// A dynamic call of a private name shows the Dart name: in the
// `NoSuchMethodError` and in `Invocation.memberName` (the production
// profile's private names carry their library's tag).

class Recorder {
  @override
  dynamic noSuchMethod(Invocation i) {
    print('nsm ${i.memberName} ${i.positionalArguments}');
    return null;
  }
}

class Known {
  int _known() => 1;
  int get _value => 2;
}

void main() {
  dynamic r = Recorder();
  r._other(1);
  r._other2;
  r._known();
  print(r._known == null);
  dynamic k = Known();
  print(k._known());
  print(k._value);
  try {
    k._missing(1);
  } on NoSuchMethodError catch (e) {
    final text = e.toString();
    print('mentions _missing: ${text.contains("'_missing'")}');
    print('shows a tag: ${text.contains(r'_missing$')}');
  }
  print(#_other == #_other);
}
