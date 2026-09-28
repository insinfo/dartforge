// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j96_host_binding_estatico.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j96_host_binding_estatico.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$J96HostBindingEstatico = const [];

class ViewJ96HostBindingEstatico0 extends import0.ComponentView<import1.J96HostBindingEstatico> {
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  late final import2.HtmlElement _el_0;
  static import3.ComponentStyles? _componentStyles;
  ViewJ96HostBindingEstatico0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('j96-host-binding-estatico'));
    this.updateChildClassNonHtml(this.rootElement, import1.J96HostBindingEstatico.hostClass);
    import7.updateAttribute(this.rootElement, 'aria-modal', import1.J96HostBindingEstatico.ariaModal);
    this.rootElement.tabIndex = import1.J96HostBindingEstatico.tabIndex;
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j96_host_binding_estatico.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendElement<import2.HtmlElement>(doc, parentRenderNode, 'i');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.indiceTexto;
    if (import8.checkBinding(this._expr_0, currVal_0, 'indiceTexto', 'asset:corpus_ngdart/lib/src/j96_host_binding_estatico.dart')) {
      import7.updateAttribute(this._el_0, 'tabindex', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j96_host_binding_estatico.dart:271:300 */;
      this._expr_0 = currVal_0;
    }
  }

  void detectHostChanges(bool firstCheck) {
    final _ctx = this.ctx;
    if (firstCheck) {
      if ((_ctx.role != null)) {
        import7.updateAttribute(this.rootElement, 'role', _ctx.role);
      }
    }
    final currVal_1 = _ctx.hostDisabled;
    if (import8.checkBinding(this._expr_1, currVal_1, null, null)) {
      import7.updateAttribute(this.rootElement, 'disabled', currVal_1);
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.indiceDoHospedeiro;
    if (import8.checkBinding(this._expr_2, currVal_2, null, null)) {
      import7.updateAttribute(this.rootElement, 'tabIndex', currVal_2);
      this._expr_2 = currVal_2;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J96HostBindingEstatico, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J96HostBindingEstaticoNgFactory = ComponentFactory<import1.J96HostBindingEstatico>('j96-host-binding-estatico', viewFactory_J96HostBindingEstaticoHost0);
ComponentFactory<import1.J96HostBindingEstatico> get J96HostBindingEstaticoNgFactory {
  return _J96HostBindingEstaticoNgFactory;
}

ComponentFactory<import1.J96HostBindingEstatico> createJ96HostBindingEstaticoFactory() {
  return ComponentFactory('j96-host-binding-estatico', viewFactory_J96HostBindingEstaticoHost0);
}

final List<Object> styles$J96HostBindingEstaticoHost = const [];

class _ViewJ96HostBindingEstaticoHost0 extends import10.HostView<import1.J96HostBindingEstatico> {
  @override
  void build() {
    this.componentView = ViewJ96HostBindingEstatico0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J96HostBindingEstatico(null);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
  }
}

import10.HostView<import1.J96HostBindingEstatico> viewFactory_J96HostBindingEstaticoHost0() {
  return _ViewJ96HostBindingEstaticoHost0();
}

final List<Object> styles$J96Usa = const [];

class ViewJ96Usa0 extends import0.ComponentView<import1.J96Usa> {
  late final ViewJ96HostBindingEstatico0 _compView_0;
  late final import1.J96HostBindingEstatico _J96HostBindingEstatico_0_5;
  static import3.ComponentStyles? _componentStyles;
  ViewJ96Usa0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('j96-usa'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j96_host_binding_estatico.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ96HostBindingEstatico0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J96HostBindingEstatico_0_5 = import1.J96HostBindingEstatico(null);
    this._compView_0.create(this._J96HostBindingEstatico_0_5);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this._compView_0.detectHostChanges(firstCheck);
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J96Usa, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J96UsaNgFactory = ComponentFactory<import1.J96Usa>('j96-usa', viewFactory_J96UsaHost0);
ComponentFactory<import1.J96Usa> get J96UsaNgFactory {
  return _J96UsaNgFactory;
}

ComponentFactory<import1.J96Usa> createJ96UsaFactory() {
  return ComponentFactory('j96-usa', viewFactory_J96UsaHost0);
}

final List<Object> styles$J96UsaHost = const [];

class _ViewJ96UsaHost0 extends import10.HostView<import1.J96Usa> {
  @override
  void build() {
    this.componentView = ViewJ96Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J96Usa();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J96Usa> viewFactory_J96UsaHost0() {
  return _ViewJ96UsaHost0();
}
