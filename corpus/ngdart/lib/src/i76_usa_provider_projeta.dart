import 'package:ngdart/angular.dart';

import 'i60_provider_use_class.dart';
import 'i75_leitor.dart';
import 'i75_provider_projeta.dart';

/// Sonda: filho com `providers:` recebendo conteúdo — outro filho com
/// provedores e uma diretiva que injeta o provedor do de fora.
@Component(
  selector: 'i76-usa-provider-projeta',
  template:
      '<i75-provider-projeta><i60-provider-use-class></i60-provider-use-class><p i75-leitor>t</p></i75-provider-projeta>',
  directives: [I75ProviderProjeta, I60ProviderUseClass, I75Leitor],
)
class I76UsaProviderProjeta {}
