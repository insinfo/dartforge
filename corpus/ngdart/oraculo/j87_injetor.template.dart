// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j87_injetor.dart';
import 'package:ngdart/src/di/injector.dart' as _i1;
import 'package:corpus_ngdart/src/j87_injetor.dart' as _i2;
import 'package:ngforms/src/directives/radio_control_value_accessor.dart' as _i3;
import 'package:ngdart/src/meta/di_tokens.dart' as _i4;
import 'package:ngdart/src/utilities.dart' as _i5;

// ignore_for_file: no_leading_underscores_for_library_prefixes
_i1.Injector injetor$Injector(_i1.Injector parent) => _Injector$injetor._(parent);

class _Injector$injetor extends _i1.HierarchicalInjector implements _i1.Injector {
  _Injector$injetor._(_i1.Injector parent) : super(parent);

  _i2.J87Servico? _field4;

  int? _field6;

  Object? _field7;

  _i2.J87Impl? _field10;

  _i3.RadioControlRegistry? _field11;

  _i2.J87Dependente? _field12;

  String _getExisting$0() => this.get(const _i4.OpaqueToken<String>('j87Nome'));

  String _getString$1() => '\u{e9} \u{1f600} \\';

  _i2.J87Cor _getJ87Cor$2() => _i2.J87Cor.verde;

  Object _getObject$3() => 3;

  _i2.J87Servico _getJ87Servico$4() => _field4 ??= _i2.J87Servico();

  Object _getExisting$5() => this.get(_i2.J87Servico);

  int _getint$6() => _field6 ??= _i2.criarNumero(
        this.get(_i2.J87Servico),
        provideUntyped(
          _i2.J87Log,
          null,
        ),
      );

  Object _getObject$7() => _field7 ??= _i2.criarLog(this.get(_i2.J87Servico));

  _i2.J87Config _getJ87Config$8() => const _i2.J87Config(
        'a',
        b: true,
        filho: _i2.J87Filho(2),
      );

  String _getString$9() => 'it\'s \$x\n';

  _i2.J87Impl _getJ87Impl$10() => _field10 ??= _i2.J87Impl.nomeado(
        this.get(_i2.J87Servico),
        provideUntyped(
          _i2.J87Log,
          null,
        ),
      );

  _i3.RadioControlRegistry _getRadioControlRegistry$11() => _field11 ??= _i3.RadioControlRegistry();

  _i2.J87Dependente _getJ87Dependente$12() => _field12 ??= _i2.J87Dependente(
        injectFromSelf(_i2.J87Servico),
        injectFromParent(_i2.J87Servico),
        injectFromAncestry(_i2.J87Servico),
        _i5.unsafeCast(injectFromAncestryOptional(
          _i2.J87Log,
          null,
        )),
        this.get(const _i4.OpaqueToken<String>('j87Nome')),
        provideUntyped(
          const _i4.OpaqueToken<String>('j87Nome'),
          null,
        ),
      );

  String _getString$13() => 'dois';

  String _getString$14() => 'um';

  String _getString$15() => 'tres';

  _i1.Injector _getInjector$16() => this;

  @override
  Object? injectFromSelfOptional(
    Object token, [
    Object? orElse = _i1.throwIfNotFound,
  ]) {
    if (identical(token, const _i4.OpaqueToken<String>('j87Apelido'))) {
      return _getExisting$0();
    }
    if (identical(token, const _i4.OpaqueToken<String>())) {
      return _getString$1();
    }
    if (identical(token, _i2.J87Enum)) {
      return _getJ87Cor$2();
    }
    if (identical(token, _i2.J87Outro)) {
      return _getObject$3();
    }
    if (identical(token, _i2.J87Servico)) {
      return _getJ87Servico$4();
    }
    if (identical(token, _i2.J87Alias)) {
      return _getExisting$5();
    }
    if (identical(token, const _i4.OpaqueToken<int>('j87Numero'))) {
      return _getint$6();
    }
    if (identical(token, _i2.J87Log)) {
      return _getObject$7();
    }
    if (identical(token, _i2.J87Config)) {
      return _getJ87Config$8();
    }
    if (identical(token, const _i4.OpaqueToken<String>('j87Nome'))) {
      return _getString$9();
    }
    if (identical(token, _i2.J87Base)) {
      return _getJ87Impl$10();
    }
    if (identical(token, _i3.RadioControlRegistry)) {
      return _getRadioControlRegistry$11();
    }
    if (identical(token, _i2.J87Dependente)) {
      return _getJ87Dependente$12();
    }
    if (identical(token, _i1.Injector)) {
      return _getInjector$16();
    }
    if (identical(token, const _i4.MultiToken<String>('j87Multi'))) {
      return [
        _getString$13(),
        _getString$14(),
        _getString$15(),
      ];
    }
    return orElse;
  }
}

_i1.Injector outro$Injector(_i1.Injector parent) => _Injector$outro._(parent);

class _Injector$outro extends _i1.HierarchicalInjector implements _i1.Injector {
  _Injector$outro._(_i1.Injector parent) : super(parent);

  _i2.J87Dependente? _field0;

  _i2.J87Log? _field1;

  _i2.J87Dependente _getJ87Dependente$0() => _field0 ??= _i2.J87Dependente(
        injectFromSelf(_i2.J87Servico),
        injectFromParent(_i2.J87Servico),
        injectFromAncestry(_i2.J87Servico),
        _i5.unsafeCast(injectFromAncestryOptional(
          _i2.J87Log,
          null,
        )),
        this.get(const _i4.OpaqueToken<String>('j87Nome')),
        provideUntyped(
          const _i4.OpaqueToken<String>('j87Nome'),
          null,
        ),
      );

  _i2.J87Log _getJ87Log$1() => _field1 ??= _i2.J87Log(this.get(_i2.J87Servico));

  String _getString$2() => 'dois';

  _i1.Injector _getInjector$3() => this;

  @override
  Object? injectFromSelfOptional(
    Object token, [
    Object? orElse = _i1.throwIfNotFound,
  ]) {
    if (identical(token, _i2.J87Dependente)) {
      return _getJ87Dependente$0();
    }
    if (identical(token, _i2.J87Log)) {
      return _getJ87Log$1();
    }
    if (identical(token, _i1.Injector)) {
      return _getInjector$3();
    }
    if (identical(token, const _i4.MultiToken<String>('j87Multi'))) {
      return [_getString$2()];
    }
    return orElse;
  }
}
