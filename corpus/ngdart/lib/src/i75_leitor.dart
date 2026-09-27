import 'package:ngdart/angular.dart';

import 'i75_provider_projeta.dart';

/// Diretiva do conteúdo projetado que injeta o provedor do componente de
/// cima (do elemento acima dela no template de quem usa).
@Directive(selector: '[i75-leitor]')
class I75Leitor {
  final I75Servico servico;
  I75Leitor(this.servico);
}
