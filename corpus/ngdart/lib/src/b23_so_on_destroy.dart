import 'package:ngdart/angular.dart';

/// Só `OnDestroy`: a hospedeira ganha o `destroyInternal`, e nenhum import
/// de `check_binding` (ninguém o lê).
@Component(
  selector: 'b23-so-on-destroy',
  templateUrl: 'b23_so_on_destroy.html',
)
class B23SoOnDestroy implements OnDestroy {
  @override
  void ngOnDestroy() {}
}
