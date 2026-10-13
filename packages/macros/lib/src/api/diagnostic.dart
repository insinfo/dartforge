part of '../api.dart';

/// A diagnostic reported by a macro. The host attaches it to the message's
/// target or, without a target, to the application's annotation.
class Diagnostic {
  /// Context messages.
  final Iterable<DiagnosticMessage> contextMessages;

  /// How to fix it, if the macro knows.
  final String? correctionMessage;

  final DiagnosticMessage message;

  final Severity severity;

  Diagnostic(this.message, this.severity,
      {List<DiagnosticMessage> contextMessages = const [],
      this.correctionMessage})
      : contextMessages = UnmodifiableListView(contextMessages);
}

/// Text and optional target of a [Diagnostic].
class DiagnosticMessage {
  final String message;

  /// Without a target, the macro application applies.
  final DiagnosticTarget? target;

  DiagnosticMessage(this.message, {this.target});
}

/// Union of the possible targets.
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
  /// Information (may not be shown).
  info,

  /// Likely problem; shown by default.
  warning,

  /// The macro could not continue; prevents compilation.
  error,
}
