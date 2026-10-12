/// Montagem da augmentation a partir dos resultados estruturados das fases.
/// Porta o contrato de `crates/macros_host/src/montagem.rs` para o builder,
/// mantendo a ordem de resultados, tipos, imports e partes do código.
library;

/// O significado de um identificador no código produzido pela macro.
enum TipoDeIdentificador { topo, estatico, instancia, local }

final class IdentificadorResolvido {
  final String nome;
  final TipoDeIdentificador tipo;
  final String? uri;
  final String? escopo;
  const IdentificadorResolvido(this.nome, this.tipo, {this.uri, this.escopo});
}

final class TipoAumentado {
  final String palavra;
  final List<String> modificadores;
  final String nome;
  final List<Map<String, Object?>> parametros;
  const TipoAumentado(this.palavra, this.modificadores, this.nome,
      [this.parametros = const []]);
}

/// Respostas semânticas necessárias à montagem. O builder as obterá do
/// `Resolver`; a execução nativa usa o equivalente em Rust.
abstract interface class ResolvedorMontagem {
  IdentificadorResolvido identificador(int id);
  TipoAumentado tipoAumentado(int id);
  Map<String, Object?>? tipoInferido(int chave);
}

/// [resultados] são os mapas de `Resultado.paraJson()`, em ordem de fase e
/// aplicação. [cabecalho] é `augment library` ou `part of`, e [uri] nomeia a
/// biblioteca original como no CFE. Falhas de resolução são erros do builder.
String montarAugmentation(
  List<Map<String, Object?>> resultados,
  ResolvedorMontagem resolvedor, {
  required String cabecalho,
  required String uri,
}) {
  if (cabecalho != 'augment library' && cabecalho != 'part of') {
    throw ArgumentError.value(cabecalho, 'cabecalho');
  }
  final m = _Montador(resolvedor);
  final valores = <int, List<Map<String, Object?>>>{};
  final extends_ = <int, Map<String, Object?>>{};
  final interfaces = <int, List<Map<String, Object?>>>{};
  final mixins = <int, List<Map<String, Object?>>>{};
  final membros = <int, List<Map<String, Object?>>>{};

  for (final resultado in resultados) {
    for (final codigo in _codigos(resultado['biblioteca'])) {
      m.codigo(codigo);
      m.texto('\n');
    }
    _acumular(valores, resultado['valoresDeEnum']);
    for (final par in _pares(resultado['extends'])) {
      final id = par[0] as int;
      if (extends_.containsKey(id)) {
        throw StateError(
            'A class cannot extend multiple classes: ${resolvedor.identificador(id).nome}');
      }
      extends_[id] = _codigo(par[1]);
    }
    _acumular(interfaces, resultado['interfaces']);
    _acumular(mixins, resultado['mixins']);
    _acumular(membros, resultado['tipos']);
  }

  // A primeira ocorrência por categoria define a ordem dos tipos.
  final tipos = <int>{
    ...valores.keys,
    ...extends_.keys,
    ...interfaces.keys,
    ...mixins.keys,
    ...membros.keys
  };
  for (final id in tipos) {
    final tipo = resolvedor.tipoAumentado(id);
    final modificadores =
        tipo.modificadores.isEmpty ? '' : '${tipo.modificadores.join(' ')} ';
    m.texto(
        'augment $modificadores${tipo.palavra} ${tipo.nome}${tipo.parametros.isEmpty ? ' ' : ''}');
    if (tipo.parametros.isNotEmpty) {
      m.texto('<');
      for (var i = 0; i < tipo.parametros.length; i++) {
        if (i > 0) m.texto(', ');
        m.codigo(tipo.parametros[i]);
      }
      m.texto('> ');
    }
    if (extends_.containsKey(id)) {
      m.texto('extends ');
      m.codigo(extends_[id]!);
      m.texto(' ');
    }
    for (final (nome, codigos) in [
      ('with', mixins[id]),
      ('implements', interfaces[id])
    ]) {
      if (codigos == null || codigos.isEmpty) continue;
      m.texto('$nome ');
      for (var i = 0; i < codigos.length; i++) {
        if (i > 0) m.texto(', ');
        m.codigo(codigos[i]);
      }
      m.texto(' ');
    }
    m.texto('{\n');
    if (tipo.palavra == 'enum') {
      for (final codigo in valores[id] ?? const <Map<String, Object?>>[]) {
        m.codigo(codigo);
      }
      m.texto(';\n');
    }
    for (final codigo in membros[id] ?? const <Map<String, Object?>>[]) {
      m.codigo(codigo);
      m.texto('\n');
    }
    m.texto('}\n');
  }
  return m.finalizar(cabecalho, uri);
}

List<List<Object?>> _pares(Object? valor) => [
      for (final par in valor as List? ?? const [])
        List<Object?>.from(par as List),
    ];

Map<String, Object?> _codigo(Object? valor) =>
    Map<String, Object?>.from(valor as Map);

List<Map<String, Object?>> _codigos(Object? valor) => [
      for (final codigo in valor as List? ?? const []) _codigo(codigo),
    ];

void _acumular(Map<int, List<Map<String, Object?>>> destino, Object? pares) {
  for (final par in _pares(pares)) {
    (destino[par[0] as int] ??= []).addAll(_codigos(par[1]));
  }
}

final class _Parte {
  final String? texto;
  final int? nome;
  const _Parte.texto(String this.texto) : nome = null;
  const _Parte.nome(int this.nome) : texto = null;

  String resolver(List<String> nomes) => texto ?? nomes[nome!];
}

final class _Montador {
  final ResolvedorMontagem resolvedor;
  final imports = <String, int>{};
  final nomes = <String>[];
  final partesDeImport = <_Parte>[];
  final partes = <_Parte>[];
  final textos = <String>[];
  final buffer = <String>[];
  String ultima = '';

  _Montador(this.resolvedor);

  void texto(String valor) {
    ultima = valor;
    buffer.add(valor);
  }

  void _descarregar() {
    for (final valor in buffer) {
      partes.add(_Parte.texto(valor));
      textos.add(valor);
    }
    buffer.clear();
  }

  void _nome(int indice) {
    _descarregar();
    ultima = '';
    partes.add(_Parte.nome(indice));
  }

  void _ident(int id) {
    final ident = resolvedor.identificador(id);
    final import = ident.uri == null
        ? null
        : imports.putIfAbsent(ident.uri!, () {
            final indice = nomes.length;
            nomes.add('');
            partesDeImport.addAll([
              _Parte.texto("import '${ident.uri}' as "),
              _Parte.nome(indice),
              const _Parte.texto(';\n'),
            ]);
            return indice;
          });
    if (ident.tipo == TipoDeIdentificador.instancia) {
      if (!ultima.trimRight().endsWith('.')) texto('this.');
    } else if (import != null) {
      _nome(import);
      texto('.');
    }
    if (ident.tipo == TipoDeIdentificador.estatico)
      texto('${ident.escopo ?? ''}.');
    texto(ident.nome);
  }

  void codigo(Map<String, Object?> codigo) {
    for (final parte in codigo['p'] as List) {
      switch (parte) {
        case String valor:
          texto(valor);
        case {'i': int id}:
          _ident(id);
        case {'o': int chave}:
          final inferido = resolvedor.tipoInferido(chave);
          if (inferido == null)
            throw StateError('nenhum tipo inferido para o tipo omitido $chave');
          this.codigo(inferido);
        case Map outro:
          this.codigo(Map<String, Object?>.from(outro));
        default:
          throw FormatException('parte de código inválida: $parte');
      }
    }
  }

  String finalizar(String cabecalho, String uri) {
    _descarregar();
    if (imports.isNotEmpty) {
      final base = _prefixoNovo(textos, 'prefix');
      for (var i = 0; i < imports.length; i++) {
        nomes[i] = '$base$i';
      }
    }
    final saida = StringBuffer("$cabecalho '$uri';\n\n");
    for (final parte in partesDeImport) {
      saida.write(parte.resolver(nomes));
    }
    if (partesDeImport.isNotEmpty) saida.write('\n');
    for (final parte in partes) {
      saida.write(parte.resolver(nomes));
    }
    return saida.toString();
  }
}

String _prefixoNovo(List<String> textos, String nome) {
  var indice = -1;
  var prefixo = nome;
  for (final texto in textos) {
    while (texto.contains(prefixo)) {
      indice++;
      prefixo = '$nome$indice';
    }
  }
  return indice > 0 ? '${prefixo}_' : prefixo;
}
