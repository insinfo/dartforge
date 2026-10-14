import 'const_record_cache/records.dart' as records;
import 'const_record_cache/plain.dart' as plain;
import 'const_record_cache/cycle_b.dart' as cycle;

Object entryRecord() {
  const value = (1, name: 'entry');
  return value;
}
Object runtimeRecord(int value) => (value, name: 'entry');

(List<int>, {String name}) explicitNested() => const ([1, 2], name: 'explicit');
((List<int>, {String label}), {String name}) explicitDeep() => const (([3], label: 'inner'), name: 'outer');

String contexts() {
  final fixed = const ([1], name: 'fixed');
  final mutable = (<int>[2], name: 'mutable');
  mutable.$1.add(3);
  var blocked = false;
  try {
    fixed.$1.add(4);
  } on UnsupportedError {
    blocked = true;
  }
  return '${mutable.$1.length}:blocked=$blocked';
}

String order = '';
int observe(String label, int value) {
  order += label;
  return value;
}
String evaluation() {
  final value = (observe('a', 1), z: observe('b', 2), a: observe('c', 3));
  return '$order:${value.$1}:${value.z}:${value.a}';
}

(int,) singleRecord() => const (6,);
(int, {String label}) firstLocal() {
  const n = 1;
  return const (n, label: 'local');
}
(int, {String label}) secondLocal() {
  const n = 2;
  return const (n, label: 'local');
}
(int, {String label}) firstImplicit() {
  const n = 1;
  const value = (n, label: 'implicit');
  return value;
}
(int, {String label}) secondImplicit() {
  const n = 2;
  const value = (n, label: 'implicit');
  return value;
}
(List<int>, Map<String, int>, Set<int>) firstCollections() {
  const n = 1;
  return const ([n], {'k': n}, {n});
}
(List<int>, Map<String, int>, Set<int>) secondCollections() {
  const n = 2;
  return const ([n], {'k': n}, {n});
}
(List<int>, Map<String, int>, Set<int>) sameCollections() {
  const n = 1;
  return const ([n], {'k': n}, {n});
}
String collections() {
  final a = firstCollections(), b = secondCollections(), same = sameCollections();
  var blocked = 0;
  try { a.$1.add(9); } on UnsupportedError { blocked++; }
  try { a.$2['k'] = 9; } on UnsupportedError { blocked++; }
  try { a.$3.add(9); } on UnsupportedError { blocked++; }
  return '${a.$1[0]}:${b.$1[0]}:${a.$2['k']}:${b.$2['k']}:${a.$3.first}:${b.$3.first}:same=${identical(a.$1, same.$1)},${identical(a.$2, same.$2)},${identical(a.$3, same.$3)}:blocked=$blocked';
}

void main() {
  print('single=${singleRecord().$1}');
  print('locals=${firstLocal().$1}:${secondLocal().$1}:${firstLocal().$1}');
  print('implicitLocals=${firstImplicit().$1}:${secondImplicit().$1}:${firstImplicit().$1}');
  print('collections=${collections()}');
  print('explicitNested=${explicitNested().$1.length}:same=${identical(explicitNested().$1, explicitNested().$1)}');
  print('explicitDeep=${explicitDeep().$1.$1.length}:same=${identical(explicitDeep().$1.$1, explicitDeep().$1.$1)}');
  print('contexts=${contexts()}');
  print('evaluation=${evaluation()}');
  print('entry=${identical(entryRecord(), entryRecord())}');
  print('runtime=${identical(runtimeRecord(1), runtimeRecord(1))}');
  print('other=${identical(records.build(), records.build())}');
  print('sameLibrary=${identical(records.build(), records.same())}');
  print('nested=${identical(records.nested(), records.nested())}');
  print('plain=${plain.value()}:same=${identical(plain.list(), plain.list())}');
  print('cycle=${cycle.value()}:same=${identical(cycle.record(), cycle.record())}');
}
