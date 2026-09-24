part of '../api.dart';

/// Lançada pela macro para parar a execução e reportar [diagnostic].
class DiagnosticException implements Exception {
  final Diagnostic diagnostic;
  DiagnosticException(this.diagnostic);
}

/// Base das exceções que o hospedeiro lança durante a execução. A macro pode
/// capturá-las para explicar melhor ao usuário; não capturadas, viram
/// diagnóstico na aplicação.
abstract interface class MacroException implements Exception {
  String get message;
  String? get stackTrace;
}

/// Algo inesperado (defeito do hospedeiro).
abstract interface class UnexpectedMacroException implements MacroException {}

/// Uso errado da API pela macro (defeito da macro). É `Exception`, não
/// `Error`, para que a macro possa capturá-la e orientar o usuário.
abstract interface class MacroImplementationException
    implements MacroException {}

/// Ciclo de introspecção entre aplicações da fase de declarações: a ordem
/// não é definida, então toda consulta do ciclo falha com esta exceção
/// (spec, "Declarations phase").
abstract interface class MacroIntrospectionCycleException
    implements MacroException {}
