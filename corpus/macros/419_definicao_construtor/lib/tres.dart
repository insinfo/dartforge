import 'package:macros/macros.dart';

macro class Tres implements ConstructorDefinitionMacro {
  const Tres();

  @override
  Future<void> buildDefinitionForConstructor(
      ConstructorDeclaration constructor,
      ConstructorDefinitionBuilder builder) async {
    builder.augment(body: FunctionBodyCode.fromString('{ valor = 3; }'));
  }
}
