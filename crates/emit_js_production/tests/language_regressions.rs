//! Production optimizations preserve observable language semantics.
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn assert_program(program: &str, expected: &str) {
    let directory = std::env::temp_dir().join(format!(
        "dartforge-language-regression-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    let source = directory.join("main.dart");
    std::fs::write(&source, program).unwrap();
    let vm = Command::new("dart").arg(&source).output().unwrap();
    assert!(
        vm.status.success(),
        "{}",
        String::from_utf8_lossy(&vm.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&vm.stdout).replace("\r\n", "\n"),
        expected
    );
    let compiler = std::env::var_os("DARTFORGE_JSPROD")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_dartforge-jsprod").into());
    for level in ["-O1", "-O2"] {
        let artifact = directory.join("main.js");
        let mut command = Command::new(&compiler);
        command.arg(&source).arg("-o").arg(&artifact).arg(level);
        if let Some(sdk) = std::env::var_os("DARTFORGE_SDK_LIB") {
            command.arg("--sdk").arg(sdk);
        }
        let compilation = command.output().unwrap();
        assert!(
            compilation.status.success(),
            "{level}: {}",
            String::from_utf8_lossy(&compilation.stderr)
        );
        let output = Command::new("node")
            .args(["-e", "globalThis.self=globalThis;require(process.argv[1])"])
            .arg(&artifact)
            .output()
            .unwrap();
        assert!(
            output.status.success() && output.stderr.is_empty(),
            "{level}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
            expected,
            "{level}"
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn lone_interpolations_remain_strings_and_keep_expression_precedence() {
    assert_program(
        r#"
const flag = true;
const folded = "$flag";
void check(Object? actual, Object? expected) {
  if (actual != expected) throw '$actual != $expected';
}
void main() {
  check(folded is String, true);
  check(folded, 'true');
  var value = false;
  check("$value", 'false');
  check("" "$value" "", 'false');
  check("$value".length, 5);
  check("$value".toUpperCase(), 'FALSE');
  var text = 'before';
  check("${text = 'ab'}".length, 2);
  check(text, 'ab');
  check("${value ? 'a' : 'xyz'}".length, 3);
  var calls = 0;
  String once() { calls++; return 'xy'; }
  check("${once()}".length, 2);
  check(calls, 1);
  check('prefix $value suffix', 'prefix false suffix');
  check('$value$value', 'falsefalse');
  print('interpolation ok');
}
"#,
        "interpolation ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn implicit_calls_keep_forwarders_without_an_explicit_selector() {
    // Extra assertions or print calls in this Dart program can retain the
    // selector through SDK code and hide the missing forwarder.
    assert_program(
        r#"
class Fallback {
  noSuchMethod(invocation) => 7;
}
class Callable {
  int call() => 1;
}
class MockCallable extends Fallback implements Callable {}
void main() {
  var value = MockCallable();
  value();
}
"#,
        "",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn nullable_late_fields_distinguish_null_from_uninitialized() {
    assert_program(
        r#"
class Holder {
  late final int? _fixed;
  late int? _mutable;
}
int? readFixed(Holder holder) => holder._fixed;
int? readMutable(Holder holder) => holder._mutable;
void writeFixed(Holder holder, int? value) { holder._fixed = value; }
void main() {
  final a = Holder(), b = Holder();
  var unreadable = false;
  try { readFixed(a); } catch (error) {
    unreadable = error.toString().startsWith('LateInitializationError:');
  }
  if (!unreadable) throw 'uninitialized read succeeded';
  writeFixed(a, null);
  if (readFixed(a) != null) throw 'initialized null';
  var repeated = false;
  try { writeFixed(a, 7); } catch (error) {
    repeated = error.toString().startsWith('LateInitializationError:');
  }
  if (!repeated || readFixed(a) != null) throw 'final null was overwritten';
  writeFixed(b, 7);
  if (readFixed(b) != 7 || readFixed(a) != null) throw 'shared field state';
  unreadable = false;
  try { readMutable(a); } catch (error) {
    unreadable = error.toString().startsWith('LateInitializationError:');
  }
  if (!unreadable) throw 'uninitialized mutable read succeeded';
  a._mutable = null;
  if (readMutable(a) != null) throw 'mutable null';
  a._mutable = 9;
  if (readMutable(a) != 9) throw 'mutable assignment';
  a._mutable = null;
  if (readMutable(a) != null) throw 'mutable reset';
  print('late null ok');
}
"#,
        "late null ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn negation_applies_to_the_whole_generated_condition() {
    assert_program(
        r#"
bool notThree(dynamic value) => !identical(value, 3);
bool notNull(dynamic value) => !identical(null, value);
bool notTrue(dynamic value) => !identical(value, true);
bool twice(dynamic value) => !(!identical(value, 3));
void main() {
  for (dynamic value in <dynamic>[3, 5, null, true, 'x']) {
    if (notThree(value) != (value != 3)) throw 'negated number';
    if (notNull(value) != (value != null)) throw 'negated null';
    if (notTrue(value) != (value != true)) throw 'negated boolean';
    if (twice(value) != (value == 3)) throw 'nested negation';
    final branch = !identical(value, 3) ? 1 : 2;
    if (branch != (value != 3 ? 1 : 2)) throw 'conditional negation';
  }
  var calls = 0;
  dynamic next() { calls++; return 5; }
  if (!identical(next(), 3) != true || calls != 1) throw 'negation effects';
  print('negation ok');
}
"#,
        "negation ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn runtime_type_recipes_remain_available_during_cache_initialization() {
    assert_program(
        r#"
import 'dart:async';
@pragma('dart2js:noInline')
bool isType<T>(dynamic value) => value is T;
@pragma('dart2js:noInline')
bool isFutureOr<T>(dynamic value) => value is FutureOr<T>;
void main() {
  print(isType<FutureOr<Never?>>(null));
  print(isFutureOr<Never?>(null));
  print(isType<FutureOr<Never>>(null));
  print(isFutureOr<Never>(null));
}
"#,
        "true\ntrue\nfalse\nfalse\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn large_field_initialization_loads_in_node_with_its_default_stack() {
    let mut program = String::from("class ManyFields {\n");
    for index in 0..=20_000 {
        program.push_str(&format!("  Object? field{index};\n"));
    }
    program.push_str(
        r#"
}
void main() {
  final value = ManyFields();
  if (value.field0 != null || value.field10000 != null || value.field20000 != null) {
    throw 'field defaults';
  }
  value.field20000 = 42;
  if (value.field20000 != 42) throw 'field assignment';
  print('large class ok');
}
"#,
    );
    assert_program(&program, "large class ok\n");
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn folded_conditionals_keep_their_static_type_and_implicit_call_boundary() {
    assert_program(
        r#"
class Base {}
class Callable extends Base {
  int calls = 0;
  int touches = 0;
  int call() => ++calls;
  void touch() { touches++; }
}
class GenericCallable {
  T call<T>(T value) => value;
}
bool get yes => true;
bool get no => false;
T context<T>(T value) => value;
void mustThrowTypeError(void Function() action) {
  var caught = false;
  try { action(); } on TypeError { caught = true; }
  if (!caught) throw 'implicit call crossed conditional boundary';
}
void main() {
  final c = Callable();
  final a = Base();
  dynamic d = null;
  mustThrowTypeError(() { context<int Function()>(no ? d : (yes ? c : a)); });
  mustThrowTypeError(() { context<int Function()>(no ? d : (no ? a : ((c)))); });
  if (c.calls != 0 || c.touches != 0) throw 'negative conversion effects';
  final f = context<int Function()>(c..touch());
  if (f() != 1 || c.touches != 1) throw 'cascade conversion';
  final g = context<int Function()>(yes ? c : c);
  if (g() != 2 || c.calls != 2) throw 'conditional conversion';
  final generic = context<int Function(int)>(yes ? GenericCallable() : GenericCallable());
  if (generic(42) != 42) throw 'generic conversion';
  if ((yes ? 1 : 2.5) + 0.5 != 1.5) throw 'numeric type';
  print('conditional calls ok');
}
"#,
        "conditional calls ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn dynamic_no_such_method_calls_keep_argument_shape_and_parameter_checks() {
    assert_program(
        r#"
class Catcher {
  int calls = 0;
  dynamic noSuchMethod(Invocation invocation) {
    calls++;
    return invocation;
  }
}
dynamic opaque(dynamic value) => value;
dynamic get invalidArgument => 42;
void main() {
  final catcher = Catcher();
  dynamic receiver = opaque(catcher);
  Invocation named = receiver.noSuchMethod(x: 42);
  if (named.memberName != #noSuchMethod || !named.isMethod ||
      named.positionalArguments.isNotEmpty || named.namedArguments[#x] != 42) {
    throw 'named invocation';
  }
  Invocation empty = receiver.noSuchMethod();
  if (empty.memberName != #noSuchMethod || empty.positionalArguments.isNotEmpty) {
    throw 'empty invocation';
  }
  Invocation extra = receiver.noSuchMethod(1, 2);
  if (extra.positionalArguments.length != 2 || extra.positionalArguments[1] != 2) {
    throw 'extra invocation';
  }
  final valid = Invocation.method(#ping, [1]);
  if (!identical(receiver.noSuchMethod(valid), valid)) throw 'valid invocation';
  if (catcher.calls != 4) throw 'handler calls';
  var caught = false;
  try { receiver.noSuchMethod(invalidArgument); } on TypeError { caught = true; }
  if (!caught || catcher.calls != 4) throw 'parameter check';
  print('noSuchMethod calls ok');
}
"#,
        "noSuchMethod calls ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn explicit_new_keeps_factory_dispatch_and_constructor_tearoffs() {
    assert_program(
        r#"
class Box {
  final int value;
  Box._(this.value);
  factory Box.new(int value) => Box._(value + 1);
  factory Box.named(int value) => Box._(value + 2);
}
abstract class Factory<T> implements Indirect<T> {
  T get value;
  factory Factory.new(T value) = Actual<T>;
}
abstract class Indirect<T> {
  T get value;
  factory Indirect.new(T value) = Factory<T>.new;
}
class Actual<T> implements Factory<T> {
  final T value;
  Actual(this.value);
}
class Value {
  final int value;
  Value.new(this.value);
  Value.named() : this.new(13);
}
void main() {
  if (Symbol.new('a') != #a || new Symbol.new('b') != #b) throw 'SDK factory';
  final symbol = Symbol.new;
  if (symbol('c') != #c) throw 'SDK factory tearoff';
  if (Box.new(1).value != 2 || Box.named(1).value != 3) throw 'factory call';
  final box = Box.new;
  if (box(3).value != 4) throw 'factory tearoff';
  if (Factory<int>.new(42).value != 42) throw 'redirecting factory';
  final factory = Factory<String>.new;
  final created = factory('ok');
  if (created.value != 'ok' || created is! Actual<String>) throw 'generic factory tearoff';
  final indirect = Indirect<String>.new;
  if (indirect('chain').value != 'chain' || indirect('chain') is! Actual<String>) {
    throw 'factory redirection chain';
  }
  if (Value.new(7).value != 7) throw 'generative call';
  if (Value.named().value != 13) throw 'generative redirection';
  final value = Value.new;
  if (value(9).value != 9) throw 'generative tearoff';
  print('explicit new ok');
}
"#,
        "explicit new ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn native_no_such_method_calls_and_tearoffs_keep_the_original_invocation() {
    assert_program(
        r#"
void main() {
  final invocation = Invocation.method(#ping, [123]);
  for (Object? receiver in [null, 777, 'hello', <String>[], true, Object(), (int x) => x]) {
    String? direct;
    try { receiver.noSuchMethod(invocation); } on NoSuchMethodError catch (error) {
      direct = error.toString();
    }
    if (direct == null || !direct.contains('ping')) throw 'original invocation lost';
    dynamic dynamicReceiver = receiver;
    dynamic tearoff = dynamicReceiver.noSuchMethod;
    String? indirect;
    try { tearoff(invocation); } on NoSuchMethodError catch (error) {
      indirect = error.toString();
    }
    if (indirect != direct) throw 'tearoff invocation differs';
  }
  print('native noSuchMethod ok');
}
"#,
        "native noSuchMethod ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn mock_accessors_preserve_final_field_storage_and_generic_checks() {
    assert_program(
        r#"
final writes = <Object?>[];
abstract class ReadWrite<T> { T get x; set x(T value); }
class Mock {
  dynamic noSuchMethod(Invocation invocation) {
    if (!invocation.isSetter || invocation.memberName != const Symbol('x=')) throw 'wrong invocation';
    writes.add(invocation.positionalArguments.single);
    return null;
  }
}
class Own extends Mock implements ReadWrite<int> { final int x = 42; }
class Base extends Mock { final int x = 42; }
class Inherited extends Base implements ReadWrite<int> {}
class GenericBase<T> extends Mock { final T x; GenericBase(this.x); }
class GenericInherited<T> extends GenericBase<T> implements ReadWrite<T> {
  GenericInherited(super.x);
}
dynamic opaque(dynamic value) => value;
void main() {
  for (ReadWrite<int> item in <ReadWrite<int>>[Own(), Inherited(), GenericInherited<int>(42)]) {
    if (writes.isNotEmpty) throw 'constructor invoked setter';
    if (item.x != 42) throw 'initial getter';
    item.x = 123;
    if (writes.length != 1 || writes.single != 123 || item.x != 42) throw 'setter storage';
    writes.clear();
    dynamic receiver = opaque(item);
    if (receiver.x != 42) throw 'dynamic getter';
    receiver.x = 124;
    if (writes.length != 1 || writes.single != 124 || receiver.x != 42) throw 'dynamic setter storage';
    writes.clear();
    var caught = false;
    try { receiver.x = 'bad'; } on TypeError { caught = true; }
    if (!caught || writes.isNotEmpty || item.x != 42) throw 'setter type check';
  }
  print('mock field storage ok');
}
"#,
        "mock field storage ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn enum_mixins_keep_constructor_arguments_after_constant_pruning() {
    assert_program(
        r#"
mixin IndexName on Enum {
  int get copiedIndex => index;
  String get copiedName => name;
}
mixin Tagged<T> on Enum { Type get argumentType => T; }
enum Sparse with IndexName { zero, unused, two }
enum Payload<T> with IndexName, Tagged<T> {
  integer<int>(7), text<String>('x');
  const Payload(this.value);
  final T value;
}
void main() {
  if (Sparse.zero.index != 0 || Sparse.two.index != 2) throw 'sparse index';
  if (Sparse.zero.copiedIndex != 0 || Sparse.two.copiedIndex != 2) throw 'mixin index';
  if (Sparse.zero.copiedName != 'zero' || Sparse.two.copiedName != 'two') throw 'mixin name';
  if (Payload.integer.index != 0 || Payload.text.index != 1) throw 'generic index';
  if (Payload.integer.value != 7 || Payload.text.value != 'x') throw 'enum fields';
  if (Payload.integer.argumentType != int || Payload.text.argumentType != String) throw 'mixin type arguments';
  if (Payload.text.copiedName != 'text' || Payload.text.copiedIndex != 1) throw 'generic mixin';
  print('enum construction ok');
}
"#,
        "enum construction ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn late_super_to_string_uses_lexical_object_implementation() {
    assert_program(
        r#"
class Parent { String toString() => 'parent'; }
class Child {
  late final String parentValue = super.toString();
  late final String Function() parentClosure = () => super.toString();
  late final String Function() parentTearoff = super.toString;
  String toString() => 'child';
}
class Grandchild extends Child { String toString() => 'grandchild'; }
void main() {
  final value = Grandchild();
  if (value.parentValue != "Instance of 'Grandchild'" || value.parentClosure() != "Instance of 'Grandchild'" || value.parentTearoff() != "Instance of 'Grandchild'") throw 'late super';
  print('late Object super ok');
}
"#,
        "late Object super ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn super_to_string_tearoffs_keep_target_and_identity() {
    assert_program(
        r#"
class Value {
  String toString() => 'override';
  String Function() parentTearoff() => super.toString;
}
void main() {
  final value = Value();
  final first = value.parentTearoff(), second = value.parentTearoff();
  if (first() != "Instance of 'Value'" || second() != first()) throw 'super tearoff target';
  if (first != second || first.hashCode != second.hashCode) throw 'super tearoff identity';
  if (first == value.toString) throw 'super tearoff override identity';
  print('super tearoff ok');
}
"#,
        "super tearoff ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn repeated_mixins_keep_lexical_super_to_string() {
    assert_program(
        r#"
class Root { String toString() => 'root'; }
mixin Layer {
  String get parentName => super.toString();
  String Function() parentTearoff() => super.toString;
}
class First extends Root with Layer { String toString() => 'first'; }
class Second extends First with Layer { String toString() => 'second'; }
void main() {
  final first = First(), second = Second();
  if (first.parentName != 'root' || first.parentTearoff()() != 'root') throw 'first layer';
  if (second.parentName != 'first' || second.parentTearoff()() != 'first') throw 'second layer';
  print('mixin super ok');
}
"#,
        "mixin super ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn folded_boolean_receivers_keep_static_extension_dispatch() {
    assert_program(
        r#"
extension ObjectOperators on Object {
  bool operator &(Object other) => false;
  bool operator |(Object other) => false;
  bool operator ^(Object other) => true;
}
const Object object = true;
const dynamic dynamicValue = true;
const bool boolean = true;
void check(bool value) { if (!value) throw 'wrong boolean operation'; }
void main() {
  check(!(object & object));
  check(!(object | object));
  check(object ^ object);
  check(dynamicValue & dynamicValue);
  check(dynamicValue | dynamicValue);
  check(!(dynamicValue ^ dynamicValue));
  check(boolean & boolean);
  check(boolean | boolean);
  check(!(boolean ^ boolean));
  if (!boolean || (boolean && false)) throw 'boolean conditions';
  print('boolean dispatch ok');
}
"#,
        "boolean dispatch ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn false_collection_loops_still_check_initializer_types() {
    assert_program(
        r#"
int checks = 0;
void throwsTypeError(Object? Function() action) {
  try { action(); } on TypeError { checks++; return; }
  throw 'missing type check';
}
void main() {
  dynamic invalid = 'string';
  throwsTypeError(() => <int>[for (int i = invalid; false;) 1]);
  throwsTypeError(() => <int, int>{for (int i = invalid; false;) 1: 1});
  throwsTypeError(() => <int>{for (int i = invalid; false;) 1});
  var effects = 0;
  int once() { effects++; return 0; }
  final values = <int>[for (int i = once(); false;) i];
  if (effects != 1 || values.isNotEmpty || checks != 3) throw 'initializer effects';
  print('collection checks ok');
}
"#,
        "collection checks ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn generic_no_such_method_getters_check_the_current_class_parameter() {
    assert_program(
        r#"
Object? result = 42;
abstract class Readable<T> { T get value; }
class Mock<T> implements Readable<T> {
  dynamic noSuchMethod(Invocation invocation) => result;
}
void main() {
  Readable<int> value = Mock<int>();
  if (value.value != 42) throw 'valid getter';
  result = 'wrong';
  var caught = false;
  try { value.value; } on TypeError { caught = true; }
  if (!caught) throw 'typed getter check';
  dynamic receiver = value;
  caught = false;
  try { receiver.value; } on TypeError { caught = true; }
  if (!caught) throw 'dynamic getter check';
  print('generic getter ok');
}
"#,
        "generic getter ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn private_mock_accessors_keep_inherited_final_field_storage() {
    assert_program(
        r#"
final writes = <Object?>[];
abstract class ReadWrite<T> { T get _x; set _x(T value); }
class Mock {
  dynamic noSuchMethod(Invocation invocation) {
    if (!invocation.isSetter) throw 'wrong invocation';
    writes.add(invocation.positionalArguments.single);
    return null;
  }
}
class Own extends Mock implements ReadWrite<int> { final int _x = 42; }
class Base extends Mock { final int _x = 42; }
class Inherited extends Base implements ReadWrite<int> {}
class GenericBase<T> extends Mock { final T _x; GenericBase(this._x); }
class GenericInherited<T> extends GenericBase<T> implements ReadWrite<T> {
  GenericInherited(super._x);
}
dynamic opaque(dynamic value) => value;
void main() {
  for (ReadWrite<int> item in <ReadWrite<int>>[Own(), Inherited(), GenericInherited<int>(42)]) {
    if (writes.isNotEmpty) throw 'constructor invoked setter';
    if (item._x != 42) throw 'initial getter';
    item._x = 123;
    if (writes.length != 1 || writes.single != 123 || item._x != 42) throw 'setter storage';
    writes.clear();
    dynamic receiver = opaque(item);
    if (receiver._x != 42) throw 'dynamic getter';
    receiver._x = 124;
    if (writes.length != 1 || writes.single != 124 || receiver._x != 42) throw 'dynamic setter storage';
    writes.clear();
    var caught = false;
    try { receiver._x = 'bad'; } on TypeError { caught = true; }
    if (!caught || writes.isNotEmpty || item._x != 42) throw 'setter type check';
  }
  print('mock field storage ok');
}
"#,
        "mock field storage ok\n",
    );
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn constant_verification_keeps_constructor_parameters_and_local_constants() {
    assert_program(
        r#"
class Pair {
  final int first, second;
  const Pair(int a, {int b = 2}) : first = a, second = b;
}
void main() {
  const initial = 1;
  const value = Pair(initial, b: initial + 1);
  if (value.first != 1 || value.second != 2) throw 'constant arguments';
  print('local constants ok');
}
"#,
        "local constants ok\n",
    );
}
