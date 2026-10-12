// O argumento de tipo `T` capturado por uma closure genérica e mandado a
// outro isolado (`Isolate.spawn`) continua o mesmo tipo lá (C11 de
// docs/NATIVO-PROJETOS-REAIS.md). O id de tipo ia como palavra crua no
// contexto da closure e cada isolado tinha a sua tabela de tipos: com
// muitos tipos internados no principal antes do spawn, o id dele apontava
// para outro tipo no filho, ou para fora da tabela (pânico no runtime — o
// `NotificationServiceWorker` do new_sali/backend).

import 'dart:isolate';

class Caixa<T> {
  final T v;
  Caixa(this.v);
}

class A {}
class B {}
class C {}

Future<void> rodar<T>(T x) async {
  final p = ReceivePort();
  await Isolate.spawn((SendPort s) {
    s.send('${Caixa<T>(x).runtimeType} ${x is T} ${<T>[].runtimeType} ${<Map<String, T>>[].runtimeType}');
  }, p.sendPort);
  print(await p.first);
}

void main() async {
  // Muitos tipos internados no isolado principal antes do spawn: os ids
  // de tipo do principal passam dos do filho.
  final ts = <Object>[
    <A>[], <B>[], <C>[], <Map<A, B>>[], <Map<B, C>>[], <Set<A>>[], <List<List<C>>>[],
    Caixa<A>(A()), Caixa<Caixa<B>>(Caixa(B())), <String, List<A>>{}, <int, Set<B>>{},
  ];
  print(ts.map((t) => t.runtimeType).join(','));
  await rodar<int>(1);
  await rodar<String>('a');
  await rodar<List<Map<String, int>>>([]);
  await rodar<Caixa<A>>(Caixa(A()));
}
