import 'package:macros/macros.dart';

macro class Gera implements ClassDeclarationsMacro {
  const Gera();

  @override
  Future<void> buildDeclarationsForClass(
      ClassDeclaration clazz, MemberDeclarationBuilder builder) async {
    builder.declareInType(DeclarationCode.fromString(
        'static const int valor = 9;'));
  }
}
