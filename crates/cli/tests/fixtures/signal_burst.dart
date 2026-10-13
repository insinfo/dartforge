// Regression: a burst of signals larger than the notification pipe, with the
// consumer stalled (the loop does not yield to the event loop), must not hang
// the signal handler; afterwards cancelling and subscribing again work.
// (The Dart VM hangs on this program: its pipe is blocking.)
import 'dart:async';
import 'dart:io';

Future<void> main() async {
  if (Platform.isWindows) {
    // No SIGUSR2 on Windows (the signals are the console events).
    print('rajada: ok');
    print('reinscricao: ok');
    return;
  }
  var recebidos = 0;
  final sub = ProcessSignal.sigusr2.watch().listen((_) => recebidos++);
  for (var i = 0; i < 200000; i++) {
    Process.killPid(pid, ProcessSignal.sigusr2);
  }
  await Future.delayed(const Duration(milliseconds: 200));
  print('rajada: ${recebidos > 0 ? 'ok' : 'nenhum sinal'}');
  await sub.cancel();

  final recebeu = Completer<void>();
  final sub2 = ProcessSignal.sigusr2.watch().listen((_) {
    if (!recebeu.isCompleted) recebeu.complete();
  });
  Process.killPid(pid, ProcessSignal.sigusr2);
  await recebeu.future.timeout(const Duration(seconds: 5));
  await sub2.cancel();
  print('reinscricao: ok');
}
