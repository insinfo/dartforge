/// Runs a macro application in one phase: picks the method from the
/// combination (target, macro interface), gives it the phase's builder and
/// turns exceptions into a diagnostic or a result exception, as the spec
/// asks ("Errors", "Exceptions thrown by macros").
library;

import 'dart:async';

import '../api.dart';
import 'model.dart';
import 'result.dart';

/// The three phases, with their protocol names.
enum Phase { types, declarations, definitions }

/// Runs [macro] on [target] in [phase] and returns the structured result.
/// Never throws: a macro failure becomes a diagnostic or the result's
/// `exception`.
Future<Result> executePhase(Macro macro, Phase phase, Object target, Introspector introspector) async {
  final result = Result();
  try {
    switch (phase) {
      case Phase.types:
        await _types(macro, target, result, introspector);
      case Phase.declarations:
        await _declarations(macro, target, result, introspector);
      case Phase.definitions:
        await _definitions(macro, target, result, introspector);
    }
  } catch (e, stack) {
    _handle(e, stack, result);
  }
  return result;
}

Never _noMatch(Macro macro, Object target) =>
    throw UnsupportedError('Unsupported macro type or invalid target:\nmacro: $macro\ntarget: $target');

Future<void> _types(Macro macro, Object target, Result r, Introspector i) async {
  final b = TypeBuilderImpl(r, i);
  switch ((target, macro)) {
    case (Library target, LibraryTypesMacro macro):
      await macro.buildTypesForLibrary(target, b);
    case (ConstructorDeclaration target, ConstructorTypesMacro macro):
      await macro.buildTypesForConstructor(target, b);
    case (MethodDeclaration target, MethodTypesMacro macro):
      await macro.buildTypesForMethod(target, b);
    case (FunctionDeclaration target, FunctionTypesMacro macro):
      await macro.buildTypesForFunction(target, b);
    case (FieldDeclaration target, FieldTypesMacro macro):
      await macro.buildTypesForField(target, b);
    case (VariableDeclaration target, VariableTypesMacro macro):
      await macro.buildTypesForVariable(target, b);
    case (ClassDeclaration target, ClassTypesMacro macro):
      await macro.buildTypesForClass(target, TargetTypeBuilderImpl(target.identifier as IdentifierImpl, r, i));
    case (EnumDeclaration target, EnumTypesMacro macro):
      await macro.buildTypesForEnum(target, TargetTypeBuilderImpl(target.identifier as IdentifierImpl, r, i));
    case (ExtensionDeclaration target, ExtensionTypesMacro macro):
      await macro.buildTypesForExtension(target, b);
    case (ExtensionTypeDeclaration target, ExtensionTypeTypesMacro macro):
      await macro.buildTypesForExtensionType(target, b);
    case (MixinDeclaration target, MixinTypesMacro macro):
      await macro.buildTypesForMixin(target, TargetTypeBuilderImpl(target.identifier as IdentifierImpl, r, i));
    case (EnumValueDeclaration target, EnumValueTypesMacro macro):
      await macro.buildTypesForEnumValue(target, b);
    case (TypeAliasDeclaration target, TypeAliasTypesMacro macro):
      await macro.buildTypesForTypeAlias(target, b);
    default:
      _noMatch(macro, target);
  }
}

/// The type that receives the declared members: the target itself, or the
/// owner of a member.
IdentifierImpl _membersType(Object target) => switch (target) {
      MemberDeclaration() => target.definingType as IdentifierImpl,
      EnumValueDeclaration() => target.definingEnum as IdentifierImpl,
      TypeDeclaration() => target.identifier as IdentifierImpl,
      _ => throw StateError('Can only create member declaration builders for types or member declarations, '
          'but got $target'),
    };

Future<void> _declarations(Macro macro, Object target, Result r, Introspector i) async {
  MemberDeclarationBuilderImpl members() => MemberDeclarationBuilderImpl(_membersType(target), r, i);
  final topLevel = DeclarationBuilderImpl(r, i);
  switch ((target, macro)) {
    case (Library target, LibraryDeclarationsMacro macro):
      await macro.buildDeclarationsForLibrary(target, topLevel);
    case (ClassDeclaration target, ClassDeclarationsMacro macro):
      await macro.buildDeclarationsForClass(target, members());
    case (EnumDeclaration target, EnumDeclarationsMacro macro):
      await macro.buildDeclarationsForEnum(target, members());
    case (ExtensionDeclaration target, ExtensionDeclarationsMacro macro):
      await macro.buildDeclarationsForExtension(target, members());
    case (ExtensionTypeDeclaration target, ExtensionTypeDeclarationsMacro macro):
      await macro.buildDeclarationsForExtensionType(target, members());
    case (MixinDeclaration target, MixinDeclarationsMacro macro):
      await macro.buildDeclarationsForMixin(target, members());
    case (EnumValueDeclaration target, EnumValueDeclarationsMacro macro):
      await macro.buildDeclarationsForEnumValue(target, members());
    case (ConstructorDeclaration target, ConstructorDeclarationsMacro macro):
      await macro.buildDeclarationsForConstructor(target, members());
    case (MethodDeclaration target, MethodDeclarationsMacro macro):
      await macro.buildDeclarationsForMethod(target, members());
    case (FieldDeclaration target, FieldDeclarationsMacro macro):
      await macro.buildDeclarationsForField(target, members());
    case (FunctionDeclaration target, FunctionDeclarationsMacro macro):
      await macro.buildDeclarationsForFunction(target, topLevel);
    case (VariableDeclaration target, VariableDeclarationsMacro macro):
      await macro.buildDeclarationsForVariable(target, topLevel);
    case (TypeAliasDeclaration target, TypeAliasDeclarationsMacro macro):
      await macro.buildDeclarationsForTypeAlias(target, topLevel);
    default:
      _noMatch(macro, target);
  }
}

Future<void> _definitions(Macro macro, Object target, Result r, Introspector i) async {
  switch ((target, macro)) {
    case (Library target, LibraryDefinitionMacro macro):
      await macro.buildDefinitionForLibrary(target, LibraryDefinitionBuilderImpl(target, r, i));
    case (ClassDeclaration target, ClassDefinitionMacro macro):
      await macro.buildDefinitionForClass(target, TypeDefinitionBuilderImpl(target, r, i));
    case (EnumDeclaration target, EnumDefinitionMacro macro):
      await macro.buildDefinitionForEnum(target, TypeDefinitionBuilderImpl(target, r, i));
    case (ExtensionDeclaration target, ExtensionDefinitionMacro macro):
      await macro.buildDefinitionForExtension(target, TypeDefinitionBuilderImpl(target, r, i));
    case (ExtensionTypeDeclaration target, ExtensionTypeDefinitionMacro macro):
      await macro.buildDefinitionForExtensionType(target, TypeDefinitionBuilderImpl(target, r, i));
    case (MixinDeclaration target, MixinDefinitionMacro macro):
      await macro.buildDefinitionForMixin(target, TypeDefinitionBuilderImpl(target, r, i));
    case (EnumValueDeclaration target, EnumValueDefinitionMacro macro):
      await macro.buildDefinitionForEnumValue(target, EnumValueDefinitionBuilderImpl(target as EnumValueImpl, r, i));
    case (ConstructorDeclaration target, ConstructorDefinitionMacro macro):
      await macro.buildDefinitionForConstructor(
          target, ConstructorDefinitionBuilderImpl(target as ConstructorImpl, r, i));
    case (MethodDeclaration target, MethodDefinitionMacro macro):
      await macro.buildDefinitionForMethod(target, FunctionDefinitionBuilderImpl(target as FunctionImpl, r, i));
    case (FieldDeclaration target, FieldDefinitionMacro macro):
      await macro.buildDefinitionForField(target, VariableDefinitionBuilderImpl(target, r, i));
    case (FunctionDeclaration target, FunctionDefinitionMacro macro):
      await macro.buildDefinitionForFunction(target, FunctionDefinitionBuilderImpl(target as FunctionImpl, r, i));
    case (VariableDeclaration target, VariableDefinitionMacro macro):
      await macro.buildDefinitionForVariable(target, VariableDefinitionBuilderImpl(target, r, i));
    default:
      _noMatch(macro, target);
  }
}

/// Macro errors: a [DiagnosticException] becomes its diagnostic; a host
/// exception ([MacroException]) becomes the result's `exception`; anything
/// else is a macro bug, reported as an error with the detail in the
/// context. `ParallelWaitError` (from `.wait` on records and lists) and
/// `AsyncError` are unwrapped.
void _handle(Object error, StackTrace stack, Result r) {
  switch (error) {
    case ParallelWaitError(:final Object? errors):
      for (final e in _errors(errors)) {
        _handle(e, stack, r);
      }
    case AsyncError():
      _handle(error.error, error.stackTrace, r);
    case DiagnosticException():
      r.diagnostics.add(error.diagnostic);
    case MacroException():
      r.exception ??= error;
    default:
      r.diagnostics.add(Diagnostic(
          DiagnosticMessage('Macro application failed due to a bug in the macro.'), Severity.error,
          contextMessages: [DiagnosticMessage('$error\n$stack')],
          correctionMessage: 'Try reporting the failure to the macro author.'));
  }
}

/// The non-null errors of a `ParallelWaitError`: a list, or a record of up
/// to 9 positions.
Iterable<Object> _errors(Object? errors) sync* {
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
