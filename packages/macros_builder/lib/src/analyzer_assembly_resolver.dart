/// Assembly resolution for simple classes already registered in the
/// builder's table. The definitions phase will add members and inferred
/// types.
library;

import 'package:analyzer/dart/element/element.dart';
import 'package:macros/src/executor/assembly.dart';

import 'identifier_table.dart';

final class AnalyzerAssemblyResolver implements AssemblyResolver {
  final IdentifierTable table;
  AnalyzerAssemblyResolver(this.table);

  @override
  ResolvedIdentifier identifier(int id) {
    final element = table.element(id);
    final generated = table.generated(id);
    if (generated != null) {
      final category = generated.category;
      return ResolvedIdentifier(
        generated.name,
        switch (category) {
          GeneratedCategory.method => IdentifierKind.instanceMember,
          GeneratedCategory.staticMember || GeneratedCategory.constructor => IdentifierKind.staticMember,
          GeneratedCategory.parameter => IdentifierKind.local,
          GeneratedCategory.type => IdentifierKind.topLevel,
        },
        uri: category == GeneratedCategory.staticMember || category == GeneratedCategory.constructor
            ? generated.uri
            : null,
        scope: generated.owner,
      );
    }
    if (element is ClassElement) {
      return ResolvedIdentifier(element.name, IdentifierKind.topLevel, uri: element.librarySource.uri.toString());
    }
    if (element is FieldElement) {
      final owner = element.enclosingElement3;
      if (owner is! ClassElement) {
        throw UnsupportedError('field without a class: $element');
      }
      final uri = element.librarySource?.uri.toString();
      if (element.isStatic && uri == null) {
        throw StateError('static field without a library: $element');
      }
      return ResolvedIdentifier(
        element.name,
        element.isStatic ? IdentifierKind.staticMember : IdentifierKind.instanceMember,
        uri: element.isStatic ? uri : null,
        scope: element.isStatic ? owner.name : null,
      );
    }
    throw UnsupportedError('identifier $id cannot be assembled yet');
  }

  @override
  AugmentedType augmentedType(int id) {
    final element = table.element(id);
    if (element is! ClassElement || element.typeParameters.isNotEmpty) {
      throw UnsupportedError('type $id cannot be assembled yet');
    }
    return AugmentedType(
        'class',
        [
          if (element.isAbstract) 'abstract',
          if (element.isBase) 'base',
          if (element.isFinal) 'final',
          if (element.isInterface) 'interface',
          if (element.isMixinClass) 'mixin',
          if (element.isSealed) 'sealed',
        ],
        element.name);
  }

  @override
  Map<String, Object?>? inferredType(int key) => null;
}
