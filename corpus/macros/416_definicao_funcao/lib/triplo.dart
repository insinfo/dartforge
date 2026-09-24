import 'package:macros/macros.dart';

macro class Triplo implements FunctionDefinitionMacro {
  const Triplo();

  @override
  Future<void> buildDefinitionForFunction(
      FunctionDeclaration function, FunctionDefinitionBuilder builder) async {
    builder.augment(FunctionBodyCode.fromString('=> valor * 3;'));
  }
}
