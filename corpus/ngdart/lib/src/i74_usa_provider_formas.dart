import 'package:ngdart/angular.dart';

import 'i60_provider_use_class.dart';
import 'i62_provider_use_factory.dart';
import 'i63_provider_use_existing.dart';
import 'i64_provider_multi.dart';
import 'i65_provider_listas.dart';

/// Sonda: filhos com `providers:` das formas que a hospedeira escreve
/// (fábrica com dependência do próprio nó, apelidos, multi, listas), um
/// deles dentro de `*ngIf` — os provedores entram no nó de quem os usa.
@Component(
  selector: 'i74-usa-provider-formas',
  templateUrl: 'i74_usa_provider_formas.html',
  directives: [
    NgIf,
    I60ProviderUseClass,
    I62ProviderUseFactory,
    I63ProviderUseExisting,
    I64ProviderMulti,
    I65ProviderListas,
  ],
)
class I74UsaProviderFormas {
  bool mostrar = true;
}
