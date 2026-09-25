import 'package:macros/macros.dart';

macro class VarExtra implements VariableDeclarationsMacro {
  const VarExtra();

  @override
  Future<void> buildDeclarationsForVariable(
      VariableDeclaration variable, DeclarationBuilder builder) async {
    builder.declareInLibrary(DeclarationCode.fromString(
        'int triplo() => base * 3;'));
  }
}
