import 'package:ngdart/angular.dart';

abstract class I60Base {}

class I60Impl implements I60Base {}

class I60Outro {}

class I60OutroImpl extends I60Outro {}

class I60Terceiro {}

/// Sonda: `ClassProvider(X, useClass: Y)`, `Provider(X, useClass: Y)` e
/// `ClassProvider(X)`, todos preguiçosos (o componente não depende deles).
@Component(
  selector: 'i60-provider-use-class',
  template: '<p>x</p>',
  providers: [
    ClassProvider(I60Base, useClass: I60Impl),
    Provider(I60Outro, useClass: I60OutroImpl),
    ClassProvider(I60Terceiro),
  ],
)
class I60ProviderUseClass {}
