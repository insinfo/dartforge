import 'dart:async';
import 'dart:ffi';
import 'dart:isolate';
import 'package:dartforge/dartforge.dart';

typedef Callback = Void Function(Pointer<Void>);
const rounds = 32;
const burst = 8;
int received = 0;
int timerCalls = 0;
int nativeCalls = 0;
int microtasks = 0;
bool closed = false;
bool finishQueued = false;
late ReceivePort inbox;
late SendPort commands;

final process = DynamicLibrary.process();
final mainQueue = process.lookup<Void>('_dispatch_main_q');
final enqueue = process.lookupFunction<
    Void Function(
        Pointer<Void>, Pointer<Void>, Pointer<NativeFunction<Callback>>),
    void Function(
        Pointer<Void>, Pointer<Void>, Pointer<NativeFunction<Callback>>)>(
  'dispatch_async_f',
);

void nativeProgress(Pointer<Void> _) {
  nativeCalls++;
  scheduleMicrotask(() {
    microtasks++;
    finishIfReady();
  });
}

void finished(Pointer<Void> _) {
  if (received != rounds * burst ||
      timerCalls != received ||
      nativeCalls != rounds ||
      microtasks != rounds ||
      !closed) {
    throw StateError('Incomplete native wake stress');
  }
  print('native pump wake stress messages=256 timers=256 native=32 validated');
  EventLoop.requestNativePumpShutdown();
}

void finishIfReady() {
  if (finishQueued ||
      !closed ||
      timerCalls != rounds * burst ||
      nativeCalls != rounds ||
      microtasks != rounds) return;
  finishQueued = true;
  inbox.close();
  // The worker port and all Dart timers are gone. Only the native pump keeps
  // the process alive until this callback requests one-turn shutdown.
  enqueue(mainQueue, nullptr, Pointer.fromFunction<Callback>(finished));
}

Future<void> worker(SendPort reply) async {
  final commands = ReceivePort();
  reply.send(commands.sendPort);
  await for (final command in commands) {
    if (command == 'close') {
      commands.close();
      reply.send('closed');
      return;
    }
    final round = command as int;
    for (var index = 0; index < burst; index++) {
      reply.send(round * burst + index);
    }
    reply.send('batch');
  }
}

void idleFinished(Pointer<Void> _) {
  print('native pump idle complete');
  EventLoop.requestNativePumpShutdown();
}

void idle(int seconds) {
  final dispatchTime = process.lookupFunction<Uint64 Function(Uint64, Int64),
      int Function(int, int)>('dispatch_time');
  final dispatchAfter = process.lookupFunction<
      Void Function(Uint64, Pointer<Void>, Pointer<Void>,
          Pointer<NativeFunction<Callback>>),
      void Function(int, Pointer<Void>, Pointer<Void>,
          Pointer<NativeFunction<Callback>>)>(
    'dispatch_after_f',
  );
  EventLoop.enableNativePump();
  dispatchAfter(dispatchTime(0, seconds * 1000000000), mainQueue, nullptr,
      Pointer.fromFunction<Callback>(idleFinished));
  // No Dart timer or port exists during this native delayed callback wait.
  print('native pump idle ready');
}

Future<void> main(List<String> args) async {
  if (!EventLoop.supportsNativePump) {
    print('native pump wake stress fallback validated');
    return;
  }
  if (args.isNotEmpty) {
    final seconds = int.parse(args.single);
    if (seconds < 1 || seconds > 30)
      throw ArgumentError('Idle seconds must be 1..30');
    idle(seconds);
    return;
  }
  inbox = ReceivePort();
  inbox.listen((value) {
    if (value is SendPort) {
      commands = value;
      commands.send(0);
    } else if (value is int) {
      if (value != received)
        throw StateError('Worker message lost or reordered');
      received++;
      final cancelled = Timer(const Duration(seconds: 60), () {
        throw StateError('Cancelled timer fired');
      });
      cancelled.cancel();
      Timer.run(() {
        timerCalls++;
        finishIfReady();
      });
    } else if (value == 'batch') {
      if (received % burst != 0) throw StateError('Incomplete worker burst');
      enqueue(
          mainQueue, nullptr, Pointer.fromFunction<Callback>(nativeProgress));
      Timer.run(() {
        if (received == rounds * burst) {
          commands.send('close');
        } else {
          commands.send(received ~/ burst);
        }
      });
    } else if (value == 'closed') {
      closed = true;
      finishIfReady();
    } else {
      throw StateError('Unexpected worker message');
    }
  });
  EventLoop.enableNativePump();
  await Isolate.spawn(worker, inbox.sendPort);
}
