// Exceções em async: throw capturado por await em try/catch, erro em then, on X, finally, rethrow, erro em Future.microtask, await em catch e em finally.
import 'dart:async';

Future<int> falha(String msg) async {
  await null;
  throw StateError(msg);
}

Future<int> falhaSincrona() async {
  throw ArgumentError('antes de qualquer await');
}

Future<int> comFinally(bool lancar) async {
  try {
    print('corpo lancar=$lancar');
    if (lancar) throw Exception('no corpo');
    return 1;
  } finally {
    print('finally lancar=$lancar');
  }
}

Future<void> comRethrow() async {
  try {
    await falha('original');
  } catch (e) {
    print('interno viu: $e');
    rethrow;
  }
}

Future<String> awaitNoCatch() async {
  try {
    throw FormatException('primeiro');
  } on FormatException catch (e) {
    print('catch: ${e.message}');
    final extra = await Future.delayed(Duration(milliseconds: 5), () => 'recuperado');
    return extra;
  }
}

Future<void> awaitNoFinally() async {
  try {
    print('corpo');
  } finally {
    await Future.delayed(Duration(milliseconds: 5));
    print('finally depois de await');
  }
}

Future<int> catchComTipos(Object erro) async {
  try {
    await Future<int>.error(erro);
    return 0;
  } on StateError catch (e) {
    print('on StateError: ${e.message}');
    return 1;
  } on ArgumentError {
    print('on ArgumentError');
    return 2;
  } catch (e) {
    print('catch genérico: $e');
    return 3;
  }
}

Future<void> main() async {
  try {
    await falha('via await');
  } catch (e) {
    print('capturado: $e');
  }
  final f = falhaSincrona();
  print('função async lançando antes de await devolve Future: ${f is Future}');
  try {
    await f;
  } catch (e) {
    print('capturado depois: ${e is ArgumentError}');
  }
  print('--');
  print('sem lançar: ${await comFinally(false)}');
  try {
    await comFinally(true);
  } catch (e) {
    print('capturado: $e');
  }
  print('--');
  try {
    await comRethrow();
  } catch (e) {
    print('externo viu: $e');
  }
  print('--');
  print(await awaitNoCatch());
  await awaitNoFinally();
  print('--');
  print(await catchComTipos(StateError('estado')));
  print(await catchComTipos(ArgumentError('arg')));
  print(await catchComTipos('string'));
  print('--');
  try {
    await Future.microtask(() => throw Exception('na microtask'));
  } catch (e) {
    print('microtask: $e');
  }
  try {
    await Future(() => throw Exception('no timer'));
  } catch (e) {
    print('timer: $e');
  }
  try {
    await Future.value(1).then((v) => throw Exception('no then'));
  } catch (e) {
    print('then: $e');
  }
  print('--');
  final log = <String>[];
  final futuro = falha('tarde').catchError((e) {
    log.add('catchError $e');
    return -1;
  });
  log.add('registrado');
  print('resultado ${await futuro} $log');
  try {
    try {
      await falha('interno');
    } finally {
      print('finally interno');
    }
  } catch (e) {
    print('externo: $e');
  }
  print('erro capturado como valor: ${await falha('x').then((v) => 'ok', onError: (e, st) => 'onError ${st.toString().isNotEmpty}')}');
  print('fim');
}
