// The runtime paths the production profile shortens: lazy statics (read,
// written, failing and retried), folded `const` ints, and the completion of
// futures with values, with other futures and in zones that intercept
// `runUnary`, in the VM's event-loop order.
import 'dart:async';

int counter = 0;
int next() => ++counter;

final int a = next();
int b = next();
var c = next() * 10;
final int fails = (counter > 100) ? 1 : throw StateError('boom');
int retry = counter < 3 ? throw StateError('early') : next();

class S {
  static final int x = next();
  static int y = next();
  static const int mask = 1 << 3;
  static const int both = mask | 1 << 1;
  static const int shifted = (both << 4) + 3 - 1;
  static List<int> list = [next()];
}

final int selfRef = selfRefInit();
int selfRefInit() => counter;

void statics() {
  print('start $counter');
  b = 5;
  print('b=$b counter=$counter');
  print('a=$a a=$a counter=$counter');
  print('c=$c c=$c');
  c = 7;
  print('c=$c');
  try {
    print(fails);
  } catch (e) {
    print('fails: $e');
  }
  try {
    print(fails);
  } catch (e) {
    print('fails again: $e');
  }
  try {
    print(retry);
  } catch (e) {
    print('retry: $e');
  }
  counter = 10;
  print('retry=$retry retry=$retry');
  print('S.y=${S.y}');
  S.y++;
  print('S.y=${S.y}');
  print('S.x=${S.x} ${S.x}');
  print('${S.mask} ${S.both} ${S.shifted}');
  S.list.add(1);
  print(S.list);
  S.list = [9];
  print(S.list);
  print('selfRef=$selfRef');
}

Future<int> value(int i) async => i;
Future<int> chained(int i) async => value(i * 2);
Future<Object> objectFuture() async => Future<int>.value(3);
Future<Future<int>> nested() async => Future<int>.value(4);
Future<int> failing() async => throw StateError('async');

Future<void> futures() async {
  final log = <String>[];
  scheduleMicrotask(() => log.add('microtask 1'));
  Timer.run(() => log.add('timer'));
  final f1 = value(1).then((v) => log.add('value $v'));
  final f2 = chained(2).then((v) => log.add('chained $v'));
  final f3 = objectFuture().then((v) => log.add('object $v'));
  final f4 = nested().then((v) => log.add('nested ${v is Future}'));
  final f5 = failing().catchError((Object e) {
    log.add('error $e');
    return 0;
  });
  scheduleMicrotask(() => log.add('microtask 2'));
  await Future.wait([f1, f2, f3, f4, f5]);
  log.add('awaited ${await value(5)} ${await chained(6)}');
  await Future<void>.delayed(Duration.zero);
  print(log.join('\n'));
  final sync = Completer<int>.sync();
  sync.future.then((v) => print('sync completer $v'));
  sync.complete(Future<int>.value(7));
  await sync.future;
  final plain = Completer<Object?>();
  plain.complete(null);
  print('plain ${await plain.future}');
}

Future<void> zones() async {
  var unary = 0;
  await runZoned(() async {
    final v = await value(8);
    final w = await Future<int>.value(9).then((x) => x + 1);
    print('in zone $v $w ${Zone.current[#name]}');
  },
      zoneValues: {#name: 'inner'},
      zoneSpecification: ZoneSpecification(
        runUnary: <R, T>(Zone self, ZoneDelegate parent, Zone zone, R Function(T) f, T arg) {
          unary++;
          return parent.runUnary(zone, f, arg);
        },
      ));
  print('runUnary intercepted ${unary > 0}');
  print('root again ${identical(Zone.current, Zone.root)}');
}

Future<void> main() async {
  statics();
  await futures();
  await zones();
}
