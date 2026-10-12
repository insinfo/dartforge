/// O serviço `macro.*` do `dfexec/1` (docs/BUILD-PROTOCOLO.md §5 e
/// docs/MACROS-PROTOCOLO.md §5), do lado do executor: recebe pedidos do
/// hospedeiro, instancia macros pelo mapa do *bootstrap*, executa as fases e
/// devolve os resultados; durante uma execução, as consultas da macro vão ao
/// hospedeiro como `macro.consulta`.
///
/// Não depende de `dart:io`: o transporte é um [Canal]. O executor nativo e
/// o builder de materialização (docs/MACROS-COMPATIBILIDADE.md) usam o canal
/// de stdio (`canal_stdio.dart`); os testes, um canal em memória.
library;

import 'dart:async';

import '../api.dart';
import 'executar.dart';
import 'modelo.dart';
import 'resultado.dart';

/// Nome e versão do protocolo (o handshake recusa outros).
const protocolo = 'dfexec/1';

/// Versão do executor de macros (a do pacote).
const versaoDoExecutor = 'dartforge-macros/0.1.3-main.0';

/// Uma fábrica de macro gerada pelo bootstrap: o construtor por *tear-off*
/// com os argumentos da anotação.
typedef FabricaDeMacro = Macro Function(List<Object?> posicionais, Map<String, Object?> nomeados);

/// Mensagens JSON já enquadradas (o enquadramento é do canal).
abstract interface class Canal {
  Stream<Map<String, Object?>> get entrada;
  void enviar(Map<String, Object?> mensagem);
  Future<void> fechar();
}

/// Atende o hospedeiro até `fim`. [macros] é o mapa do bootstrap:
/// `'uri#Classe'` → construtor (`''` para o sem nome) → fábrica.
Future<void> servir(Map<String, Map<String, FabricaDeMacro>> macros, Canal canal, {String abi = ''}) async {
  final servico = _Servico(macros, canal, abi);
  await for (final m in canal.entrada) {
    if (!servico.receber(m)) break;
  }
  await Future.wait(servico.emCurso);
  await canal.fechar();
}

final class _Servico {
  final Map<String, Map<String, FabricaDeMacro>> macros;
  final Canal canal;
  final String abi;
  final instancias = <int, Macro>{};
  final _consultas = <int, Completer<Object?>>{};
  final emCurso = <Future<void>>[];
  var _proximaInstancia = 1;
  var _proximaConsulta = 1;
  _Servico(this.macros, this.canal, this.abi);

  void _erro(Object? id, String mensagem) => canal.enviar({'t': 'erro', 'id': id, 'mensagem': mensagem});

  /// Trata uma mensagem; `false` encerra.
  bool receber(Map<String, Object?> m) {
    final id = m['id'];
    switch (m['t']) {
      case 'ola':
        if (m['protocolo'] != protocolo) {
          _erro(null, 'protocolo ${m['protocolo']} não suportado (este executor fala $protocolo)');
          return false;
        }
        canal.enviar({
          't': 'ola',
          'protocolo': protocolo,
          'servicos': ['macro'],
          'executor': versaoDoExecutor,
          'abi': abi,
          'macros': [
            for (final MapEntry(:key, :value) in macros.entries)
              {'macro': key, 'construtores': value.keys.toList()},
          ],
        });
      case 'macro.instanciar':
        try {
          final fabrica = macros[m['macro']]?[m['construtor'] ?? ''];
          if (fabrica == null) {
            _erro(id, 'macro ${m['macro']} (construtor "${m['construtor']}") fora do bootstrap');
            return true;
          }
          final args = m['argumentos'] as Map<String, Object?>? ?? const {};
          final modelo = Modelo(_SemHospedeiro());
          final macro = fabrica(
            [for (final a in args['posicionais'] as List? ?? const []) argumentoDeJson(a, modelo)],
            {
              for (final MapEntry(:key, :value) in (args['nomeados'] as Map<String, Object?>? ?? const {}).entries)
                key: argumentoDeJson(value, modelo),
            },
          );
          final n = _proximaInstancia++;
          instancias[n] = macro;
          canal.enviar({'t': 'macro.instancia', 'id': id, 'instancia': n, 'interfaces': interfacesDe(macro)});
        } catch (e) {
          _erro(id, 'falha ao instanciar ${m['macro']}: $e');
        }
      case 'macro.executar':
        emCurso.add(_executar(m));
      case 'macro.resposta':
        final c = _consultas.remove(id);
        final erro = m['erro'] as Map<String, Object?>?;
        if (c == null) break;
        if (erro != null) {
          c.completeError(ErroDoHospedeiro(erro['tipo'] as String? ?? 'inesperado', erro['mensagem'] as String? ?? ''));
        } else {
          c.complete(m['valor']);
        }
      case 'macro.descartar':
        instancias.remove(m['instancia']);
      case 'fim':
        canal.enviar({'t': 'fim'});
        return false;
      default:
        _erro(id, 'mensagem desconhecida: ${m['t']}');
    }
    return true;
  }

  Future<void> _executar(Map<String, Object?> m) async {
    final id = m['id'];
    final macro = instancias[m['instancia']];
    if (macro == null) {
      _erro(id, 'instância ${m['instancia']} inexistente');
      return;
    }
    final fase = Fase.values.byName(m['fase'] as String);
    final modelo = Modelo(_PeloCanal(this, id as int));
    modelo.receber(m['modelo'] as Map<String, Object?>? ?? const {});
    final alvoJson = m['alvo'] as Map<String, Object?>;
    final Object alvo = alvoJson['k'] == 'biblioteca' ? modelo.biblioteca(alvoJson) : modelo.declaracao(alvoJson);
    final r = await executarFase(macro, fase, alvo, Introspector(modelo));
    canal.enviar({'t': 'macro.resultado', 'id': id, 'resultado': r.paraJson()});
  }

  Future<Object?> consultar(int execucao, String tipo, Map<String, Object?> args) {
    final n = _proximaConsulta++;
    final c = Completer<Object?>();
    _consultas[n] = c;
    canal.enviar({'t': 'macro.consulta', 'id': n, 'execucao': execucao, 'tipo': tipo, 'args': args});
    return c.future;
  }
}

final class _PeloCanal implements Hospedeiro {
  final _Servico servico;
  final int execucao;
  _PeloCanal(this.servico, this.execucao);

  @override
  Future<Object?> consultar(String tipo, Map<String, Object?> args) => servico.consultar(execucao, tipo, args);
}

/// Instanciar não consulta nada: argumentos só literais, `Code` e tipos.
final class _SemHospedeiro implements Hospedeiro {
  @override
  Future<Object?> consultar(String tipo, Map<String, Object?> args) =>
      Future.error(ErroDoHospedeiro('implementacao', 'consulta fora de uma execução'));
}

/// Um argumento de aplicação (spec, "Macro arguments"): literal, lista,
/// conjunto, mapa, `Code` ou anotação de tipo, com a etiqueta do tipo.
Object? argumentoDeJson(Object? j, Modelo m) {
  final a = j as Map<String, Object?>;
  final v = a['v'];
  return switch (a['t']) {
    'null' => null,
    'bool' => v as bool,
    'int' => int.parse(v as String),
    'double' => (v as num).toDouble(),
    'string' => v as String,
    'lista' => [for (final x in v as List) argumentoDeJson(x, m)],
    'set' => {for (final x in v as List) argumentoDeJson(x, m)},
    'mapa' => {
        for (final par in v as List) argumentoDeJson((par as List)[0], m): argumentoDeJson(par[1], m),
      },
    'codigo' => codigoDeJson(v, m),
    'tipo' => m.tipo(v as Map<String, Object?>).code,
    final t => throw ExcecaoInesperada('argumento desconhecido: $t'),
  };
}

/// As interfaces de macro que [m] implementa, pelo nome: o hospedeiro decide
/// com elas em que fases a aplicação roda (o `shouldExecute` da 1ª geração).
List<String> interfacesDe(Macro m) => [
      if (m is LibraryTypesMacro) 'LibraryTypesMacro',
      if (m is LibraryDeclarationsMacro) 'LibraryDeclarationsMacro',
      if (m is LibraryDefinitionMacro) 'LibraryDefinitionMacro',
      if (m is FunctionTypesMacro) 'FunctionTypesMacro',
      if (m is FunctionDeclarationsMacro) 'FunctionDeclarationsMacro',
      if (m is FunctionDefinitionMacro) 'FunctionDefinitionMacro',
      if (m is VariableTypesMacro) 'VariableTypesMacro',
      if (m is VariableDeclarationsMacro) 'VariableDeclarationsMacro',
      if (m is VariableDefinitionMacro) 'VariableDefinitionMacro',
      if (m is ClassTypesMacro) 'ClassTypesMacro',
      if (m is ClassDeclarationsMacro) 'ClassDeclarationsMacro',
      if (m is ClassDefinitionMacro) 'ClassDefinitionMacro',
      if (m is EnumTypesMacro) 'EnumTypesMacro',
      if (m is EnumDeclarationsMacro) 'EnumDeclarationsMacro',
      if (m is EnumDefinitionMacro) 'EnumDefinitionMacro',
      if (m is EnumValueTypesMacro) 'EnumValueTypesMacro',
      if (m is EnumValueDeclarationsMacro) 'EnumValueDeclarationsMacro',
      if (m is EnumValueDefinitionMacro) 'EnumValueDefinitionMacro',
      if (m is FieldTypesMacro) 'FieldTypesMacro',
      if (m is FieldDeclarationsMacro) 'FieldDeclarationsMacro',
      if (m is FieldDefinitionMacro) 'FieldDefinitionMacro',
      if (m is MethodTypesMacro) 'MethodTypesMacro',
      if (m is MethodDeclarationsMacro) 'MethodDeclarationsMacro',
      if (m is MethodDefinitionMacro) 'MethodDefinitionMacro',
      if (m is ConstructorTypesMacro) 'ConstructorTypesMacro',
      if (m is ConstructorDeclarationsMacro) 'ConstructorDeclarationsMacro',
      if (m is ConstructorDefinitionMacro) 'ConstructorDefinitionMacro',
      if (m is MixinTypesMacro) 'MixinTypesMacro',
      if (m is MixinDeclarationsMacro) 'MixinDeclarationsMacro',
      if (m is MixinDefinitionMacro) 'MixinDefinitionMacro',
      if (m is ExtensionTypesMacro) 'ExtensionTypesMacro',
      if (m is ExtensionDeclarationsMacro) 'ExtensionDeclarationsMacro',
      if (m is ExtensionDefinitionMacro) 'ExtensionDefinitionMacro',
      if (m is ExtensionTypeTypesMacro) 'ExtensionTypeTypesMacro',
      if (m is ExtensionTypeDeclarationsMacro) 'ExtensionTypeDeclarationsMacro',
      if (m is ExtensionTypeDefinitionMacro) 'ExtensionTypeDefinitionMacro',
      if (m is TypeAliasTypesMacro) 'TypeAliasTypesMacro',
      if (m is TypeAliasDeclarationsMacro) 'TypeAliasDeclarationsMacro',
    ];
