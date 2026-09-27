// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j02_host_binding_so_herdado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j02_host_binding_so_herdado.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$J02HostBindingSoHerdado = const [];

class ViewJ02HostBindingSoHerdado0 extends import0.ComponentView<import1.J02HostBindingSoHerdado> {
  Object? _expr_0;
  static import2.ComponentStyles? _componentStyles;
  ViewJ02HostBindingSoHerdado0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j02-host-binding-so-herdado'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j02_host_binding_so_herdado.dart' : null);
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
      if ((_ctx.rotulo != null)) {
        import7.updateAttribute(this.rootElement, 'aria-label', _ctx.rotulo);
      }
    }
    final currVal_0 = _ctx.base;
    if (import8.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateClassBindingNonHtml(this.rootElement, 'base', currVal_0);
      this._expr_0 = currVal_0;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J02HostBindingSoHerdado, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J02HostBindingSoHerdadoNgFactory = ComponentFactory<import1.J02HostBindingSoHerdado>('j02-host-binding-so-herdado', viewFactory_J02HostBindingSoHerdadoHost0);
ComponentFactory<import1.J02HostBindingSoHerdado> get J02HostBindingSoHerdadoNgFactory {
  return _J02HostBindingSoHerdadoNgFactory;
}

ComponentFactory<import1.J02HostBindingSoHerdado> createJ02HostBindingSoHerdadoFactory() {
  return ComponentFactory('j02-host-binding-so-herdado', viewFactory_J02HostBindingSoHerdadoHost0);
}

final List<Object> styles$J02HostBindingSoHerdadoHost = const [];

class _ViewJ02HostBindingSoHerdadoHost0 extends import10.HostView<import1.J02HostBindingSoHerdado> {
  @override
  void build() {
    this.componentView = ViewJ02HostBindingSoHerdado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J02HostBindingSoHerdado();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
  }
}

import10.HostView<import1.J02HostBindingSoHerdado> viewFactory_J02HostBindingSoHerdadoHost0() {
  return _ViewJ02HostBindingSoHerdadoHost0();
}
