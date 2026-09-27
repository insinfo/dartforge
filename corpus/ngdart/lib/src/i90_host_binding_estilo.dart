import 'package:ngdart/angular.dart';

/// Sonda: `@HostBinding('style.x')` e `@HostBinding('style.x.px')`.
@Component(
  selector: 'i90-host-binding-estilo',
  templateUrl: 'i90_host_binding_estilo.html',
)
class I90HostBindingEstilo {
  @HostBinding('style.color')
  String cor = 'red';
  @HostBinding('style.width.px')
  int largura = 10;
  @HostBinding('style.opacity')
  double? opacidade;
}
