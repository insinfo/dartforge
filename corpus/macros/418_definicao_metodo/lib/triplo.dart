import 'package:macros/macros.dart';

macro class Triplo implements MethodDefinitionMacro {
  const Triplo();

  @override
  Future<void> buildDefinitionForMethod(
      MethodDeclaration method, FunctionDefinitionBuilder builder) async {
    builder.augment(FunctionBodyCode.fromString('=> valor * 3;'));
  }
}
