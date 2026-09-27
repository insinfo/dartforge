// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i89_host_binding_propriedade.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i89_host_binding_propriedade.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$I89HostBindingPropriedade = const [];

class ViewI89HostBindingPropriedade0 extends import0.ComponentView<import1.I89HostBindingPropriedade> {
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  static import2.ComponentStyles? _componentStyles;
  ViewI89HostBindingPropriedade0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i89-host-binding-propriedade'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i89_host_binding_propriedade.dart' : null);
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
    if (firstCheck) {
      if ((_ctx.escondido != null)) {
        import7.setProperty(this.rootElement, 'hidden', _ctx.escondido);
      }
    }
    final currVal_0 = _ctx.titulo;
    if (import8.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.setProperty(this.rootElement, 'title', currVal_0);
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.indice;
    if (import8.checkBinding(this._expr_1, currVal_1, null, null)) {
      import7.setProperty(this.rootElement, 'tabIndex', currVal_1);
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.id;
    if (import8.checkBinding(this._expr_2, currVal_2, null, null)) {
      import7.setProperty(this.rootElement, 'id', currVal_2);
      this._expr_2 = currVal_2;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I89HostBindingPropriedade, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I89HostBindingPropriedadeNgFactory = ComponentFactory<import1.I89HostBindingPropriedade>('i89-host-binding-propriedade', viewFactory_I89HostBindingPropriedadeHost0);
ComponentFactory<import1.I89HostBindingPropriedade> get I89HostBindingPropriedadeNgFactory {
  return _I89HostBindingPropriedadeNgFactory;
}

ComponentFactory<import1.I89HostBindingPropriedade> createI89HostBindingPropriedadeFactory() {
  return ComponentFactory('i89-host-binding-propriedade', viewFactory_I89HostBindingPropriedadeHost0);
}

final List<Object> styles$I89HostBindingPropriedadeHost = const [];

class _ViewI89HostBindingPropriedadeHost0 extends import10.HostView<import1.I89HostBindingPropriedade> {
  @override
  void build() {
    this.componentView = ViewI89HostBindingPropriedade0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I89HostBindingPropriedade();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
  }
}

import10.HostView<import1.I89HostBindingPropriedade> viewFactory_I89HostBindingPropriedadeHost0() {
  return _ViewI89HostBindingPropriedadeHost0();
}
