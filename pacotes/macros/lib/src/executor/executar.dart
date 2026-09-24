/// Executa uma aplicação de macro numa fase: escolhe o método pela
/// combinação (alvo, interface da macro), dá a ela o builder da fase e
/// converte as exceções em diagnóstico ou exceção de resultado, como a spec
/// pede ("Errors", "Exceptions thrown by macros").
library;

import 'dart:async';

import '../api.dart';
import 'modelo.dart';
import 'resultado.dart';

/// As três fases, com os nomes do protocolo.
enum Fase { tipos, declaracoes, definicoes }

/// Roda [macro] sobre [alvo] em [fase] e devolve o resultado estruturado.
/// Nunca lança: falha da macro vira diagnóstico ou `excecao` do resultado.
Future<Resultado> executarFase(Macro macro, Fase fase, Object alvo, Introspector introspector) async {
  final resultado = Resultado();
  try {
    switch (fase) {
      case Fase.tipos:
        await _tipos(macro, alvo, resultado, introspector);
      case Fase.declaracoes:
        await _declaracoes(macro, alvo, resultado, introspector);
      case Fase.definicoes:
        await _definicoes(macro, alvo, resultado, introspector);
    }
  } catch (e, pilha) {
    _tratar(e, pilha, resultado);
  }
  return resultado;
}

Never _semCasamento(Macro macro, Object alvo) =>
    throw UnsupportedError('Unsupported macro type or invalid target:\nmacro: $macro\ntarget: $alvo');

Future<void> _tipos(Macro macro, Object alvo, Resultado r, Introspector i) async {
  final b = ConstrutorDeTipos(r, i);
  switch ((alvo, macro)) {
    case (Library alvo, LibraryTypesMacro macro):
      await macro.buildTypesForLibrary(alvo, b);
    case (ConstructorDeclaration alvo, ConstructorTypesMacro macro):
      await macro.buildTypesForConstructor(alvo, b);
    case (MethodDeclaration alvo, MethodTypesMacro macro):
      await macro.buildTypesForMethod(alvo, b);
    case (FunctionDeclaration alvo, FunctionTypesMacro macro):
      await macro.buildTypesForFunction(alvo, b);
    case (FieldDeclaration alvo, FieldTypesMacro macro):
      await macro.buildTypesForField(alvo, b);
    case (VariableDeclaration alvo, VariableTypesMacro macro):
      await macro.buildTypesForVariable(alvo, b);
    case (ClassDeclaration alvo, ClassTypesMacro macro):
      await macro.buildTypesForClass(alvo, ConstrutorDeTiposDoAlvo(alvo.identifier as IdentificadorImpl, r, i));
    case (EnumDeclaration alvo, EnumTypesMacro macro):
      await macro.buildTypesForEnum(alvo, ConstrutorDeTiposDoAlvo(alvo.identifier as IdentificadorImpl, r, i));
    case (ExtensionDeclaration alvo, ExtensionTypesMacro macro):
      await macro.buildTypesForExtension(alvo, b);
    case (ExtensionTypeDeclaration alvo, ExtensionTypeTypesMacro macro):
      await macro.buildTypesForExtensionType(alvo, b);
    case (MixinDeclaration alvo, MixinTypesMacro macro):
      await macro.buildTypesForMixin(alvo, ConstrutorDeTiposDoAlvo(alvo.identifier as IdentificadorImpl, r, i));
    case (EnumValueDeclaration alvo, EnumValueTypesMacro macro):
      await macro.buildTypesForEnumValue(alvo, b);
    case (TypeAliasDeclaration alvo, TypeAliasTypesMacro macro):
      await macro.buildTypesForTypeAlias(alvo, b);
    default:
      _semCasamento(macro, alvo);
  }
}

/// O tipo que recebe os membros declarados: o próprio alvo, ou o dono de um
/// membro.
IdentificadorImpl _tipoDosMembros(Object alvo) => switch (alvo) {
      MemberDeclaration() => alvo.definingType as IdentificadorImpl,
      EnumValueDeclaration() => alvo.definingEnum as IdentificadorImpl,
      TypeDeclaration() => alvo.identifier as IdentificadorImpl,
      _ => throw StateError('Can only create member declaration builders for types or member declarations, '
          'but got $alvo'),
    };

Future<void> _declaracoes(Macro macro, Object alvo, Resultado r, Introspector i) async {
  ConstrutorDeMembros membros() => ConstrutorDeMembros(_tipoDosMembros(alvo), r, i);
  final topo = ConstrutorDeDeclaracoes(r, i);
  switch ((alvo, macro)) {
    case (Library alvo, LibraryDeclarationsMacro macro):
      await macro.buildDeclarationsForLibrary(alvo, topo);
    case (ClassDeclaration alvo, ClassDeclarationsMacro macro):
      await macro.buildDeclarationsForClass(alvo, membros());
    case (EnumDeclaration alvo, EnumDeclarationsMacro macro):
      await macro.buildDeclarationsForEnum(alvo, membros());
    case (ExtensionDeclaration alvo, ExtensionDeclarationsMacro macro):
      await macro.buildDeclarationsForExtension(alvo, membros());
    case (ExtensionTypeDeclaration alvo, ExtensionTypeDeclarationsMacro macro):
      await macro.buildDeclarationsForExtensionType(alvo, membros());
    case (MixinDeclaration alvo, MixinDeclarationsMacro macro):
      await macro.buildDeclarationsForMixin(alvo, membros());
    case (EnumValueDeclaration alvo, EnumValueDeclarationsMacro macro):
      await macro.buildDeclarationsForEnumValue(alvo, membros());
    case (ConstructorDeclaration alvo, ConstructorDeclarationsMacro macro):
      await macro.buildDeclarationsForConstructor(alvo, membros());
    case (MethodDeclaration alvo, MethodDeclarationsMacro macro):
      await macro.buildDeclarationsForMethod(alvo, membros());
    case (FieldDeclaration alvo, FieldDeclarationsMacro macro):
      await macro.buildDeclarationsForField(alvo, membros());
    case (FunctionDeclaration alvo, FunctionDeclarationsMacro macro):
      await macro.buildDeclarationsForFunction(alvo, topo);
    case (VariableDeclaration alvo, VariableDeclarationsMacro macro):
      await macro.buildDeclarationsForVariable(alvo, topo);
    case (TypeAliasDeclaration alvo, TypeAliasDeclarationsMacro macro):
      await macro.buildDeclarationsForTypeAlias(alvo, topo);
    default:
      _semCasamento(macro, alvo);
  }
}

Future<void> _definicoes(Macro macro, Object alvo, Resultado r, Introspector i) async {
  switch ((alvo, macro)) {
    case (Library alvo, LibraryDefinitionMacro macro):
      await macro.buildDefinitionForLibrary(alvo, ConstrutorDeDefinicaoDeBiblioteca(alvo, r, i));
    case (ClassDeclaration alvo, ClassDefinitionMacro macro):
      await macro.buildDefinitionForClass(alvo, ConstrutorDeDefinicaoDeTipo(alvo, r, i));
    case (EnumDeclaration alvo, EnumDefinitionMacro macro):
      await macro.buildDefinitionForEnum(alvo, ConstrutorDeDefinicaoDeTipo(alvo, r, i));
    case (ExtensionDeclaration alvo, ExtensionDefinitionMacro macro):
      await macro.buildDefinitionForExtension(alvo, ConstrutorDeDefinicaoDeTipo(alvo, r, i));
    case (ExtensionTypeDeclaration alvo, ExtensionTypeDefinitionMacro macro):
      await macro.buildDefinitionForExtensionType(alvo, ConstrutorDeDefinicaoDeTipo(alvo, r, i));
    case (MixinDeclaration alvo, MixinDefinitionMacro macro):
      await macro.buildDefinitionForMixin(alvo, ConstrutorDeDefinicaoDeTipo(alvo, r, i));
    case (EnumValueDeclaration alvo, EnumValueDefinitionMacro macro):
      await macro.buildDefinitionForEnumValue(alvo, ConstrutorDeDefinicaoDeValor(alvo as ValorDeEnumImpl, r, i));
    case (ConstructorDeclaration alvo, ConstructorDefinitionMacro macro):
      await macro.buildDefinitionForConstructor(
          alvo, ConstrutorDeDefinicaoDeConstrutor(alvo as ConstrutorImpl, r, i));
    case (MethodDeclaration alvo, MethodDefinitionMacro macro):
      await macro.buildDefinitionForMethod(alvo, ConstrutorDeDefinicaoDeFuncao(alvo as FuncaoImpl, r, i));
    case (FieldDeclaration alvo, FieldDefinitionMacro macro):
      await macro.buildDefinitionForField(alvo, ConstrutorDeDefinicaoDeVariavel(alvo, r, i));
    case (FunctionDeclaration alvo, FunctionDefinitionMacro macro):
      await macro.buildDefinitionForFunction(alvo, ConstrutorDeDefinicaoDeFuncao(alvo as FuncaoImpl, r, i));
    case (VariableDeclaration alvo, VariableDefinitionMacro macro):
      await macro.buildDefinitionForVariable(alvo, ConstrutorDeDefinicaoDeVariavel(alvo, r, i));
    default:
      _semCasamento(macro, alvo);
  }
}

/// Erros da macro: [DiagnosticException] vira o diagnóstico dela; exceção
/// do hospedeiro ([MacroException]) vira a `excecao` do resultado; qualquer
/// outra coisa é defeito da macro, reportado como erro com o detalhe no
/// contexto. `ParallelWaitError` (do `.wait` de records e listas) e
/// `AsyncError` são desembrulhados.
void _tratar(Object erro, StackTrace pilha, Resultado r) {
  switch (erro) {
    case ParallelWaitError(:final Object? errors):
      for (final e in _erros(errors)) {
        _tratar(e, pilha, r);
      }
    case AsyncError():
      _tratar(erro.error, erro.stackTrace, r);
    case DiagnosticException():
      r.diagnosticos.add(erro.diagnostic);
    case MacroException():
      r.excecao ??= erro;
    default:
      r.diagnosticos.add(Diagnostic(
          DiagnosticMessage('Macro application failed due to a bug in the macro.'), Severity.error,
          contextMessages: [DiagnosticMessage('$erro\n$pilha')],
          correctionMessage: 'Try reporting the failure to the macro author.'));
  }
}

/// Os erros não nulos de um `ParallelWaitError`: lista, ou record de até 9
/// posições.
Iterable<Object> _erros(Object? errors) sync* {
  switch (errors) {
    case List<Object?> l:
      yield* l.nonNulls;
    case (final a,):
      yield* [a].nonNulls;
    case (final a, final b):
      yield* [a, b].nonNulls;
    case (final a, final b, final c):
      yield* [a, b, c].nonNulls;
    case (final a, final b, final c, final d):
      yield* [a, b, c, d].nonNulls;
    case (final a, final b, final c, final d, final e):
      yield* [a, b, c, d, e].nonNulls;
    case (final a, final b, final c, final d, final e, final f):
      yield* [a, b, c, d, e, f].nonNulls;
    case (final a, final b, final c, final d, final e, final f, final g):
      yield* [a, b, c, d, e, f, g].nonNulls;
    case (final a, final b, final c, final d, final e, final f, final g, final h):
      yield* [a, b, c, d, e, f, g, h].nonNulls;
    case (final a, final b, final c, final d, final e, final f, final g, final h, final i):
      yield* [a, b, c, d, e, f, g, h, i].nonNulls;
    default:
      if (errors != null) yield errors;
  }
}
