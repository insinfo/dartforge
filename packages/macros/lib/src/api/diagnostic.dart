part of '../api.dart';

/// Um diagnóstico reportado por uma macro. O hospedeiro o liga ao alvo da
/// mensagem ou, sem alvo, à anotação da aplicação.
class Diagnostic {
  /// Mensagens de contexto.
  final Iterable<DiagnosticMessage> contextMessages;

  /// Como corrigir, se a macro souber.
  final String? correctionMessage;

  final DiagnosticMessage message;

  final Severity severity;

  Diagnostic(this.message, this.severity,
      {List<DiagnosticMessage> contextMessages = const [],
      this.correctionMessage})
      : contextMessages = UnmodifiableListView(contextMessages);
}

/// Texto e alvo opcional de um [Diagnostic].
class DiagnosticMessage {
  final String message;

  /// Sem alvo, vale a aplicação da macro.
  final DiagnosticTarget? target;

  DiagnosticMessage(this.message, {this.target});
}

/// União dos alvos possíveis.
sealed class DiagnosticTarget {}

final class DeclarationDiagnosticTarget extends DiagnosticTarget {
  final Declaration declaration;

  DeclarationDiagnosticTarget(this.declaration);
}

extension DeclarationAsTarget on Declaration {
  DeclarationDiagnosticTarget get asDiagnosticTarget =>
      DeclarationDiagnosticTarget(this);
}

final class TypeAnnotationDiagnosticTarget extends DiagnosticTarget {
  final TypeAnnotation typeAnnotation;

  TypeAnnotationDiagnosticTarget(this.typeAnnotation);
}

extension TypeAnnotationAsTarget on TypeAnnotation {
  TypeAnnotationDiagnosticTarget get asDiagnosticTarget =>
      TypeAnnotationDiagnosticTarget(this);
}

final class MetadataAnnotationDiagnosticTarget extends DiagnosticTarget {
  final MetadataAnnotation metadataAnnotation;

  MetadataAnnotationDiagnosticTarget(this.metadataAnnotation);
}

extension MetadataAnnotationAsTarget on MetadataAnnotation {
  MetadataAnnotationDiagnosticTarget get asDiagnosticTarget =>
      MetadataAnnotationDiagnosticTarget(this);
}

enum Severity {
  /// Informação (pode não ser mostrada).
  info,

  /// Provável problema; mostrado por padrão.
  warning,

  /// A macro não pôde seguir; impede a compilação.
  error,
}
