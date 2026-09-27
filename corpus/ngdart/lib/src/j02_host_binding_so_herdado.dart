import 'package:ngdart/angular.dart';

/// Base com `@HostBinding`, sem anotação de componente.
abstract class J02Base {
  @HostBinding('class.base')
  bool base = true;
  @HostBinding('attr.aria-label')
  final String rotulo = 'r';
}

/// Sonda: componente sem `@HostBinding` próprio que herda os da base.
@Component(
  selector: 'j02-host-binding-so-herdado',
  templateUrl: 'j02_host_binding_so_herdado.html',
)
class J02HostBindingSoHerdado extends J02Base {}
