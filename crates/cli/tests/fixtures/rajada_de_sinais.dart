// Regressão: uma rajada de sinais maior que o pipe de notificação, com o
// consumidor parado (o laço não cede ao laço de eventos), não pode travar o
// tratador de sinais; depois o cancelamento e uma nova inscrição funcionam.
// (A VM do Dart trava neste programa: o pipe dela é bloqueante.)
import 'dart:async';
import 'dart:io';

Future<void> main() async {
  if (Platform.isWindows) {
    // Sem SIGUSR2 no Windows (os sinais são os eventos de console).
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
