// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j136_construtor_anotado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j136_construtor_anotado.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/di/errors.dart' as import10;
import 'package:ngdart/src/meta/di_tokens.dart' as import11;
import 'dart:core';

final List<Object> styles$J136Caixa = const [];

class ViewJ136Caixa0 extends import0.ComponentView<import1.J136Caixa> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ136Caixa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j136-caixa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j136_construtor_anotado.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _text_0 = import7.appendText(parentRenderNode, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J136Caixa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J136CaixaNgFactory = ComponentFactory<import1.J136Caixa>('j136-caixa', viewFactory_J136CaixaHost0);
ComponentFactory<import1.J136Caixa> get J136CaixaNgFactory {
  return _J136CaixaNgFactory;
}

ComponentFactory<import1.J136Caixa> createJ136CaixaFactory() {
  return ComponentFactory('j136-caixa', viewFactory_J136CaixaHost0);
}

final List<Object> styles$J136CaixaHost = const [];

class _ViewJ136CaixaHost0 extends import9.HostView<import1.J136Caixa> {
  @override
  void build() {
    this.componentView = ViewJ136Caixa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J136Caixa, () {
            return import1.J136Caixa(this.injectorGetOptional(import1.J136Gerador, this.parentIndex), this.injectorGetOptional(import1.J136Tamanho, this.parentIndex), this.injectorGetOptional(const import11.OpaqueToken<bool>('j136'), this.parentIndex), null, null, this.componentView, _el_0);
          })
        : import1.J136Caixa(this.injectorGetOptional(import1.J136Gerador, this.parentIndex), this.injectorGetOptional(import1.J136Tamanho, this.parentIndex), this.injectorGetOptional(const import11.OpaqueToken<bool>('j136'), this.parentIndex), null, null, this.componentView, _el_0));
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J136Caixa> viewFactory_J136CaixaHost0() {
  return _ViewJ136CaixaHost0();
}
