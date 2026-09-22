// exceções customizadas: implements Exception com toString, hierarquia, Error vs Exception, campos, ordem dos on.
class AppException implements Exception {
  final String mensagem;
  final int codigo;
  const AppException(this.mensagem, [this.codigo = 0]);
  @override
  String toString() => 'AppException($codigo): $mensagem';
}

class NaoEncontrado extends AppException {
  final String recurso;
  const NaoEncontrado(this.recurso) : super('não encontrado: $recurso', 404);
}

class NaoAutorizado extends AppException {
  const NaoAutorizado() : super('não autorizado', 401);
  @override
  String toString() => 'Sem permissão';
}

class ValidacaoException implements Exception {
  final Map<String, String> erros;
  ValidacaoException(this.erros);
  @override
  String toString() => 'Validação falhou: ${erros.keys.join(', ')}';
}

class MeuErro extends Error {
  final String detalhe;
  MeuErro(this.detalhe);
  @override
  String toString() => 'MeuErro: $detalhe';
}

class SemToString implements Exception {}

class Semaforo {
  final String cor;
  Semaforo(this.cor);
}

void buscar(String id) {
  if (id.isEmpty) throw ValidacaoException({'id': 'obrigatório'});
  if (id == 'x') throw NaoEncontrado('item $id');
  if (id == 'admin') throw NaoAutorizado();
  if (id == 'bug') throw MeuErro('estado inconsistente');
  if (id == 'raw') throw Semaforo('vermelho');
  print('achou $id');
}

String trata(String id) {
  try {
    buscar(id);
    return 'ok';
  } on NaoEncontrado catch (e) {
    return '404 ${e.recurso} / ${e.codigo}';
  } on AppException catch (e) {
    return 'app ${e.codigo}: $e';
  } on ValidacaoException catch (e) {
    return 'validação ${e.erros}';
  } on Error catch (e) {
    return 'erro: $e';
  } catch (e) {
    return 'outro: ${e is Semaforo ? (e as Semaforo).cor : e}';
  }
}

void main() {
  for (final id in ['abc', 'x', 'admin', '', 'bug', 'raw']) {
    print(trata(id));
  }

  // on Base pega Derivada
  try {
    throw NaoEncontrado('r');
  } on AppException catch (e) {
    print('base pegou derivada: ${e.mensagem}');
    print(e is NaoEncontrado);
    print(e is NaoAutorizado);
  }

  // on Exception pega implementações de Exception
  try {
    throw AppException('m', 1);
  } on Exception catch (e) {
    print('on Exception: $e');
  }

  // Error vs Exception
  try {
    throw MeuErro('e');
  } on Exception catch (_) {
    print('não é Exception');
  } on Error catch (e) {
    print('é Error: $e');
  }
  print(MeuErro('x') is Exception);
  print(MeuErro('x') is Error);
  print(AppException('x') is Exception);
  print(AppException('x') is Error);

  // toString padrão de classe sem override que implementa Exception
  final s = SemToString();
  print(s.toString() == "Instance of 'SemToString'");
  print(s is Exception);

  // const exceções são canônicas
  const a = AppException('c', 1);
  const b = AppException('c', 1);
  print(identical(a, b));
  try {
    throw a;
  } catch (e) {
    print(identical(e, b));
  }

  // campos acessíveis via on tipado
  try {
    throw ValidacaoException({'nome': 'curto', 'idade': 'negativa'});
  } on ValidacaoException catch (e) {
    print(e.erros.length);
    print(e.erros['idade']);
    print(e);
  }

  // Error com stackTrace preenchido só após throw
  final erro = MeuErro('antes');
  print(erro.stackTrace == null);
  try {
    throw erro;
  } catch (e) {
    print((e as MeuErro).stackTrace != null);
  }

  // hierarquia com on em ordem errada: o primeiro pega tudo (o mais específico nunca roda)
  try {
    throw NaoAutorizado();
  } on AppException catch (e) {
    print('genérico primeiro: ${e.codigo}');
  } on NaoAutorizado catch (_) {
    print('nunca');
  }

  // exceção como valor em lista e relançada
  final erros = <Exception>[AppException('um'), NaoEncontrado('dois'), SemToString()];
  for (final ex in erros) {
    try {
      throw ex;
    } on NaoEncontrado catch (e) {
      print('NF ${e.codigo}');
    } on AppException catch (e) {
      print('AE ${e.codigo}');
    } on Exception {
      print('EX');
    }
  }

  // exceção com toString que usa interpolação de campos calculados
  final v = ValidacaoException({});
  print(v);
  print('fim');
}
