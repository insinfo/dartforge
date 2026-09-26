// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i17_host_binding_componente.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i17_host_binding_componente.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$I17HostBindingComponente = const [];

class ViewI17HostBindingComponente0 extends import0.ComponentView<import1.I17HostBindingComponente> {
  Object? _expr_0;
  Object? _expr_1;
  static import2.ComponentStyles? _componentStyles;
  ViewI17HostBindingComponente0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i17-host-binding-componente'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i17_host_binding_componente.dart' : null);
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
    final currVal_0 = _ctx.ativo;
    if (import8.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateClassBindingNonHtml(this.rootElement, 'ativo', currVal_0);
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.papel;
    if (import8.checkBinding(this._expr_1, currVal_1, null, null)) {
      import7.updateAttribute(this.rootElement, 'role', currVal_1);
      this._expr_1 = currVal_1;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I17HostBindingComponente, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I17HostBindingComponenteNgFactory = ComponentFactory<import1.I17HostBindingComponente>('i17-host-binding-componente', viewFactory_I17HostBindingComponenteHost0);
ComponentFactory<import1.I17HostBindingComponente> get I17HostBindingComponenteNgFactory {
  return _I17HostBindingComponenteNgFactory;
}

ComponentFactory<import1.I17HostBindingComponente> createI17HostBindingComponenteFactory() {
  return ComponentFactory('i17-host-binding-componente', viewFactory_I17HostBindingComponenteHost0);
}

final List<Object> styles$I17HostBindingComponenteHost = const [];

class _ViewI17HostBindingComponenteHost0 extends import10.HostView<import1.I17HostBindingComponente> {
  @override
  void build() {
    this.componentView = ViewI17HostBindingComponente0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I17HostBindingComponente();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
  }
}

import10.HostView<import1.I17HostBindingComponente> viewFactory_I17HostBindingComponenteHost0() {
  return _ViewI17HostBindingComponenteHost0();
}
