import 'package:ngdart/angular.dart';

/// Filho com todos os ganchos de ciclo de vida e dois `@Input`: quem o usa
/// chama cada gancho no lugar do oficial (`lifecycle_binder.dart`).
@Component(
  selector: 'd05-filho-ciclo',
  templateUrl: 'd05_filho_ciclo.html',
)
class D05FilhoCiclo
    implements
        OnInit,
        DoCheck,
        AfterChanges,
        AfterContentInit,
        AfterContentChecked,
        AfterViewInit,
        AfterViewChecked,
        OnDestroy {
  @Input()
  String titulo = '';

  @Input()
  int contador = 0;

  @override
  void ngOnInit() {}

  @override
  void ngDoCheck() {}

  @override
  void ngAfterChanges() {}

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
