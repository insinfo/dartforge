import 'package:ngdart/angular.dart';

/// `@HostListener` com argumentos que não são `$event`: o oficial gera um
/// método `_handleEvent_N` — forma que o gerador ainda recusa.
@Component(
  selector: 'b17-host-listener-com-args',
  templateUrl: 'b17_host_listener_com_args.html',
)
class B17HostListenerComArgs {
  @HostListener('input', [r'$event.target'])
  void mudou(Object? alvo) {}
}