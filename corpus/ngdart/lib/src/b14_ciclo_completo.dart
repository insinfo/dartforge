import 'package:ngdart/angular.dart';

/// Todos os ganchos juntos: a ordem e o agrupamento dos `if` no
/// `detectChangesInternal` da visão-hospedeira.
@Component(
  selector: 'b14-ciclo-completo',
  templateUrl: 'b14_ciclo_completo.html',
)
class B14CicloCompleto
    implements
        OnInit,
        DoCheck,
        AfterContentInit,
        AfterContentChecked,
        AfterViewInit,
        AfterViewChecked,
        OnDestroy {
  @override
  void ngOnInit() {}

  @override
  void ngDoCheck() {}

  @override
  void ngAfterContentInit() {}

  @override
  void ngAfterContentChecked() {}

  @override
  void ngAfterViewInit() {}

  @override
  void ngAfterViewChecked() {}

  @override
  void ngOnDestroy() {}
}
