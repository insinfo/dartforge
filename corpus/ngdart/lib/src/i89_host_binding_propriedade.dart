import 'package:ngdart/angular.dart';

/// Sonda: `@HostBinding` de propriedade (`tabIndex`, `title`), sem
/// argumento (o nome do membro) e `final`.
@Component(
  selector: 'i89-host-binding-propriedade',
  templateUrl: 'i89_host_binding_propriedade.html',
)
class I89HostBindingPropriedade {
  @HostBinding('tabIndex')
  int indice = 0;
  @HostBinding('title')
  String get titulo => 't';
  @HostBinding()
  String id = 'x';
  @HostBinding('hidden')
  final bool escondido = false;
}
