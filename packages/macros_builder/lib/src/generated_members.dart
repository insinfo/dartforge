/// Reads declarations of the partial augmentation without depending on
/// analyzer 7.3 associating `import augment`. Only the members the builder
/// can serialize enter the definitions-phase model.
library;

import 'package:analyzer/dart/analysis/features.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/analysis/utilities.dart';
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/element/element.dart';

import 'identifier_table.dart';

Future<Map<String, Object?>> generatedMembers(
  String source,
  ClassElement classElement,
  Map<String, Object?> execution,
  IdentifierTable table,
) async {
  final parsed = parseString(
    content: source,
    featureSet: FeatureSet.latestLanguageVersion(flags: ['macros']),
    throwIfDiagnostics: false,
  );
  if (parsed.errors.isNotEmpty) {
    throw FormatException('invalid partial augmentation: ${parsed.errors}');
  }
  final unit = parsed.unit;
  final imports = <String, String>{};
  for (final directive in unit.directives.whereType<ImportDirective>()) {
    final prefix = directive.prefix?.name;
    final uri = directive.uri.stringValue;
    if (prefix != null && uri != null) imports[prefix] = uri;
  }
  final declarations =
      unit.declarations.whereType<ClassDeclaration>().where((d) => d.name.lexeme == classElement.name).toList();
  if (declarations.length != 1) {
    throw StateError('augmentation of ${classElement.name} missing or duplicated');
  }
  final declaration = declarations.single;
  final target = Map<String, Object?>.from(execution['target'] as Map);
  final ident = Map<String, Object?>.from(target['ident'] as Map);
  final lib = Map<String, Object?>.from(target['lib'] as Map);
  final targetUri = lib['uri'] as String;

  Future<Map<String, Object?>> type(TypeAnnotation input) async {
    if (input is! NamedType) {
      throw UnsupportedError('generated type is not named: ${input.toSource()}');
    }
    final prefix = input.importPrefix?.name.lexeme;
    final uri = prefix == null ? targetUri : imports[prefix];
    if (uri == null) {
      throw StateError('prefix $prefix is not imported in the augmentation');
    }
    final result = await classElement.library.session.getLibraryByUri(uri);
    if (result is! LibraryElementResult) {
      throw StateError('library $uri not resolved by the analyzer');
    }
    final name = input.name2.lexeme;
    final elements = result.element.topLevelElements.whereType<ClassElement>().where((e) => e.name == name);
    if (elements.isEmpty) throw StateError('type $uri#$name not found');
    final args = <Map<String, Object?>>[];
    for (final argument in input.typeArguments?.arguments ?? const <TypeAnnotation>[]) {
      args.add(await type(argument));
    }
    return {
      't': 'named',
      'ident': {'id': table.id(elements.first), 'name': name},
      'args': args,
      if (input.question != null) 'nullable': true,
    };
  }

  Future<Map<String, Object?>> parameter(FormalParameter p, String ownerKey) async {
    final normal = p is DefaultFormalParameter ? p.parameter : p;
    if (normal is! SimpleFormalParameter || normal.type == null || normal.name == null) {
      throw UnsupportedError('generated parameter not covered: ${p.toSource()}');
    }
    final name = normal.name!.lexeme;
    return {
      'k': 'parameter',
      'ident': {
        'id': table.generatedId('parameter:$ownerKey:$name',
            meaning: GeneratedIdentifier(name, targetUri, GeneratedCategory.parameter)),
        'name': name,
      },
      'lib': lib,
      'type': await type(normal.type!),
      'named': p.isNamed,
      'required': p.isRequired,
      'style': 'normal',
    };
  }

  final methods = <Map<String, Object?>>[];
  final constructors = <Map<String, Object?>>[];
  // The host serializes methods before constructors, even though the AST puts
  // the constructor first; this also sets the order of new IDs.
  for (final method in declaration.members.whereType<MethodDeclaration>()) {
    if (method.returnType == null || method.parameters == null) {
      throw UnsupportedError('generated method without a return type or parameters');
    }
    final name = method.name.lexeme;
    final key = '$targetUri#${classElement.name}.$name';
    final positional = <Map<String, Object?>>[];
    final named = <Map<String, Object?>>[];
    for (final p in method.parameters!.parameters) {
      final j = await parameter(p, 'method:$key');
      (p.isNamed ? named : positional).add(j);
    }
    methods.add({
      'k': 'method',
      'ident': {
        'id': table.generatedId('method:$key',
            meaning: GeneratedIdentifier(
                name, targetUri, method.isStatic ? GeneratedCategory.staticMember : GeneratedCategory.method,
                owner: classElement.name)),
        'name': name
      },
      'lib': lib,
      'owner': ident,
      'static': method.isStatic,
      'body': true,
      'external': method.externalKeyword != null,
      'operator': method.isOperator,
      'getter': method.isGetter,
      'setter': method.isSetter,
      'return': await type(method.returnType!),
      'positional': positional,
      'named': named,
      'tparams': <Object?>[],
    });
  }
  for (final ctor in declaration.members.whereType<ConstructorDeclaration>()) {
    final name = ctor.name?.lexeme ?? '';
    final key = '$targetUri#${classElement.name}.$name';
    final positional = <Map<String, Object?>>[];
    final named = <Map<String, Object?>>[];
    for (final p in ctor.parameters.parameters) {
      final j = await parameter(p, 'constructor:$key');
      (p.isNamed ? named : positional).add(j);
    }
    constructors.add({
      'k': 'constructor',
      'ident': {
        'id': table.generatedId('constructor:$key',
            meaning: GeneratedIdentifier(name, targetUri, GeneratedCategory.constructor, owner: classElement.name)),
        'name': name
      },
      'lib': lib,
      'owner': ident,
      'body': true,
      'external': ctor.externalKeyword != null,
      'const': ctor.constKeyword != null,
      'factory': ctor.factoryKeyword != null,
      'return': {'t': 'omitted', 'key': table.omitted('constructor:$key:return')},
      'positional': positional,
      'named': named,
      'tparams': <Object?>[],
    });
  }
  return {'methods': methods, 'constructors': constructors};
}
