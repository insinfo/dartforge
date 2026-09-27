import 'package:ngdart/angular.dart';

class I67Local {
  final NgZone zona;
  I67Local(this.zona);
}

class I67Externo {}

abstract class I67Apelido {}

/// Sonda: provedor que depende de algo de fora do nó (vem do injetor) e
/// apelido de um token que o nó não provê.
@Component(
  selector: 'i67-provider-externo',
  template: '<p>x</p>',
  providers: [
    ClassProvider(I67Local),
    ExistingProvider(I67Apelido, I67Externo),
  ],
)
class I67ProviderExterno {}
