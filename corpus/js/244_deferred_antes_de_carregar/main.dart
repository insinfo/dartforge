// diverge-ddc: na web (DDC) a biblioteca deferida conta como carregada já na chamada de
// loadLibrary() e o erro é DeferredNotLoadedError com a URI da biblioteca; na VM, só
// quando o Future completa, e _DeferredNotLoadedError.
// (Cópia de corpus/native/147_deferred_antes_de_carregar.)
// `import … deferred as p`: acessar `p` antes de `p.loadLibrary()` completar
// lança `_DeferredNotLoadedError` (o `CheckLibraryIsLoaded` do front end da
// VM), em chamada, leitura, escrita, incremento, tear-off, construtor e membro
// estático; dois prefixos da mesma biblioteca carregam separadamente; a
// biblioteca conta como carregada quando o `Future` completa, num turno do laço
// de eventos depois de um timer zero agendado logo após a chamada.
import 'dart:async';

import 'biblioteca.dart' deferred as a;
import 'biblioteca.dart' deferred as b;

bool carregada() {
  try {
    a.lista;
    return true;
  } catch (_) {
    return false;
  }
}

void tenta(String nome, void Function() f) {
  try {
    f();
    print('$nome: ok');
  } catch (e) {
    // O texto do DDC traz a URI da biblioteca, que depende do build.
    print('$nome: ${e.runtimeType}');
    print('  NoSuchMethodError: ${e is NoSuchMethodError}, Error: ${e is Error}');
  }
}

void main() async {
  tenta('chamada', () => a.saudacao('x'));
  tenta('leitura', () => a.lista);
  tenta('escrita', () => a.contador = 1);
  tenta('incremento', () => a.contador++);
  tenta('tear-off', () => a.saudacao);
  tenta('construtor', () => a.Ponto(1));
  tenta('new', () => new a.Ponto(2));
  tenta('estático', () => a.Ponto.criados);

  scheduleMicrotask(() => print('microtarefa antes: ${carregada()}'));
  final f = a.loadLibrary();
  scheduleMicrotask(() => print('microtarefa depois: ${carregada()}'));
  Timer.run(() => print('timer: ${carregada()}'));
  f.then((_) => print('then: ${carregada()}'));
  print('síncrono: ${carregada()}');
  print('resultado: ${await f}');
  print('await: ${carregada()}');

  print(a.saudacao('mundo'));
  a.contador += 5;
  a.Ponto.criados++;
  print('${a.contador} ${a.Ponto.criados} ${a.Ponto(7).x}');
  final t = a.saudacao;
  print(t('tear-off'));
  print(identical(f, a.loadLibrary()));

  tenta('outro prefixo', () => b.lista);
  await b.loadLibrary();
  print('b: ${b.contador}');
}
