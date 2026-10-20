import 'dart:ffi';

int callback(int value) {
  final text = 'callback:$value';
  return value + text.length;
}

void main() {
  final library = DynamicLibrary.process();
  final absolute = library.lookupFunction<Int32 Function(Int32), int Function(int)>('abs');
  final pointer = Pointer.fromFunction<Int64 Function(Int64)>(callback, 0);
  final invoke = pointer.asFunction<int Function(int)>();
  print(absolute(-37));
  print(invoke(21));
}
