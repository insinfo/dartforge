// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i91_host_binding_heranca.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i91_host_binding_heranca.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$I91HostBindingHeranca = const [];

class ViewI91HostBindingHeranca0 extends import0.ComponentView<import1.I91HostBindingHeranca> {
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  static import2.ComponentStyles? _componentStyles;
  ViewI91HostBindingHeranca0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i91-host-binding-heranca'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i91_host_binding_heranca.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  void detectHostChanges(bool firstCheck) {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.papel;
    if (import8.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateAttribute(this.rootElement, 'role', currVal_0);
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.base;
    if (import8.checkBinding(this._expr_1, currVal_1, null, null)) {
      import7.updateClassBindingNonHtml(this.rootElement, 'base', currVal_1);
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.proprio;
    if (import8.checkBinding(this._expr_2, currVal_2, null, null)) {
      import7.updateClassBindingNonHtml(this.rootElement, 'proprio', currVal_2);
      this._expr_2 = currVal_2;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I91HostBindingHeranca, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I91HostBindingHerancaNgFactory = ComponentFactory<import1.I91HostBindingHeranca>('i91-host-binding-heranca', viewFactory_I91HostBindingHerancaHost0);
ComponentFactory<import1.I91HostBindingHeranca> get I91HostBindingHerancaNgFactory {
  return _I91HostBindingHerancaNgFactory;
}

ComponentFactory<import1.I91HostBindingHeranca> createI91HostBindingHerancaFactory() {
  return ComponentFactory('i91-host-binding-heranca', viewFactory_I91HostBindingHerancaHost0);
}

final List<Object> styles$I91HostBindingHerancaHost = const [];

class _ViewI91HostBindingHerancaHost0 extends import10.HostView<import1.I91HostBindingHeranca> {
  @override
  void build() {
    this.componentView = ViewI91HostBindingHeranca0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I91HostBindingHeranca();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
  }
}

import10.HostView<import1.I91HostBindingHeranca> viewFactory_I91HostBindingHerancaHost0() {
  return _ViewI91HostBindingHerancaHost0();
}
