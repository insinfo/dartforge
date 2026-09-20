import 'dart:ffi' as ffi;
@ffi.Native<ffi.Int32 Function(ffi.Int32)>(symbol: 'fixture_echo32')
external int echo32(int value);
@ffi.Native<ffi.Int64 Function(ffi.Int64)>(symbol: 'fixture_echo64')
external int echo64(int value);
@ffi.Native<ffi.Void Function(ffi.Int64)>(symbol: 'fixture_store')
external void store(int value);
@ffi.Native<ffi.Int64 Function()>(symbol: 'fixture_load')
external int load();
@ffi.Native<ffi.Int64 Function(ffi.Int64, ffi.Int64)>(symbol: 'fixture_pair')
external int pair(int a, int b);
int step(int value) { store(value); return load(); }
void main() {
  print(echo32(-1));
  print(echo32(1073741824 * 2));
  print(echo32(1073741824 * 4 + 7));
  print(echo64(1073741824 * 4 + 7));
  store(-42);
  print(load());
  print(pair(step(1), step(2)));
  print(load());
}
