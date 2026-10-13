part of '../api.dart';

/// Thrown by the macro to stop the execution and report [diagnostic].
class DiagnosticException implements Exception {
  final Diagnostic diagnostic;
  DiagnosticException(this.diagnostic);
}

/// Base of the exceptions the host throws during the execution. The macro may
/// catch them to explain better to the user; uncaught, they become a
/// diagnostic on the application.
abstract interface class MacroException implements Exception {
  String get message;
  String? get stackTrace;
}

/// Something unexpected (a host bug).
abstract interface class UnexpectedMacroException implements MacroException {}

/// Wrong use of the API by the macro (a macro bug). It is an `Exception`, not
/// an `Error`, so that the macro can catch it and guide the user.
abstract interface class MacroImplementationException
    implements MacroException {}

/// Introspection cycle between applications of the declarations phase: the
/// order is undefined, so every query of the cycle fails with this exception
/// (spec, "Declarations phase").
abstract interface class MacroIntrospectionCycleException
    implements MacroException {}
