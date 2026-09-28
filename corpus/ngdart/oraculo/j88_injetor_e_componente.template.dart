// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j88_injetor_e_componente.dart';
import 'package:ngdart/src/di/injector.dart' as _i1;
import 'package:corpus_ngdart/src/j88_injetor_e_componente.dart' as _i2;
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j88_injetor_e_componente.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$J88Raiz = const [];

class ViewJ88Raiz0 extends import0.ComponentView<import1.J88Raiz> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewJ88Raiz0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j88-raiz'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j88_injetor_e_componente.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.texto)) /* REF:asset:corpus_ngdart/lib/src/j88_injetor_e_componente.dart:218:229 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J88Raiz, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J88RaizNgFactory = ComponentFactory<import1.J88Raiz>('j88-raiz', viewFactory_J88RaizHost0);
ComponentFactory<import1.J88Raiz> get J88RaizNgFactory {
  return _J88RaizNgFactory;
}

ComponentFactory<import1.J88Raiz> createJ88RaizFactory() {
  return ComponentFactory('j88-raiz', viewFactory_J88RaizHost0);
}

final List<Object> styles$J88RaizHost = const [];

class _ViewJ88RaizHost0 extends import11.HostView<import1.J88Raiz> {
  @override
  void build() {
    this.componentView = ViewJ88Raiz0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J88Raiz();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.J88Raiz> viewFactory_J88RaizHost0() {
  return _ViewJ88RaizHost0();
}

// ignore_for_file: no_leading_underscores_for_library_prefixes
_i1.Injector injetor$Injector(_i1.Injector parent) => _Injector$injetor._(parent);

class _Injector$injetor extends _i1.HierarchicalInjector implements _i1.Injector {
  _Injector$injetor._(_i1.Injector parent) : super(parent);

  _i2.J88Servico? _field0;

  _i2.J88Servico _getJ88Servico$0() => _field0 ??= _i2.J88Servico();

  _i1.Injector _getInjector$1() => this;

  @override
  Object? injectFromSelfOptional(
    Object token, [
    Object? orElse = _i1.throwIfNotFound,
  ]) {
    if (identical(token, _i2.J88Servico)) {
      return _getJ88Servico$0();
    }
    if (identical(token, _i1.Injector)) {
      return _getInjector$1();
    }
    return orElse;
  }
}
