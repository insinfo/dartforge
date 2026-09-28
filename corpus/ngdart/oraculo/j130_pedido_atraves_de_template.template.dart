// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j130_pedido_atraves_de_template.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j130_pedido_atraves_de_template.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import8;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;

final List<Object> styles$J130Caixa = const [];

class ViewJ130Caixa0 extends import0.ComponentView<import1.J130Caixa> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ130Caixa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j130-caixa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j130_pedido_atraves_de_template.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this.project(parentRenderNode, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J130Caixa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J130CaixaNgFactory = ComponentFactory<import1.J130Caixa>('j130-caixa', viewFactory_J130CaixaHost0);
ComponentFactory<import1.J130Caixa> get J130CaixaNgFactory {
  return _J130CaixaNgFactory;
}

ComponentFactory<import1.J130Caixa> createJ130CaixaFactory() {
  return ComponentFactory('j130-caixa', viewFactory_J130CaixaHost0);
}

final List<Object> styles$J130CaixaHost = const [];

class _ViewJ130CaixaHost0 extends import8.HostView<import1.J130Caixa> {
  late dynamic _J130A_0_6 = import1.fabricaA();
  late dynamic _J130B_0_7 = import1.fabricaB();
  @override
  void build() {
    this.componentView = ViewJ130Caixa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J130Caixa();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import1.J130A)) {
        return this._J130A_0_6;
      }
      if (identical(token, import1.J130B)) {
        return this._J130B_0_7;
      }
    }
    return notFoundResult;
  }
}

import8.HostView<import1.J130Caixa> viewFactory_J130CaixaHost0() {
  return _ViewJ130CaixaHost0();
}

final List<Object> styles$J130Usa = const [];

class ViewJ130Usa0 extends import0.ComponentView<import1.J130Usa> {
  late dynamic _J130B_0_6 = import1.fabricaB();
  late dynamic _J130A_0_7 = import1.fabricaA();
  late final ViewJ130Caixa0 _compView_0;
  late final import1.J130Caixa _J130Caixa_0_5;
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  static import2.ComponentStyles? _componentStyles;
  ViewJ130Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j130-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j130_pedido_atraves_de_template.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ130Caixa0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J130Caixa_0_5 = import1.J130Caixa();
    final _anchor_1 = import11.createAnchor();
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J130Usa1);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
    this._compView_0.createAndProject(this._J130Caixa_0_5, [
      <Object>[this._appEl_1]
    ]);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((nodeIndex <= 1)) {
      if (identical(token, import1.J130B)) {
        return this._J130B_0_6;
      }
      if (identical(token, import1.J130A)) {
        return this._J130A_0_7;
      }
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.x);
    }
    this._NgIf_1_9.ngIf = _ctx.x /* REF:asset:corpus_ngdart/lib/src/j130_pedido_atraves_de_template.dart:761:770 */;
    this._appEl_1.detectChangesInNestedViews();
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J130Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J130UsaNgFactory = ComponentFactory<import1.J130Usa>('j130-usa', viewFactory_J130UsaHost0);
ComponentFactory<import1.J130Usa> get J130UsaNgFactory {
  return _J130UsaNgFactory;
}

ComponentFactory<import1.J130Usa> createJ130UsaFactory() {
  return ComponentFactory('j130-usa', viewFactory_J130UsaHost0);
}

class _ViewJ130Usa1 extends import14.EmbeddedView<import1.J130Usa> {
  late final import1.J130Pede _J130Pede_1_5;
  _ViewJ130Usa1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('div'));
    final _el_1 = import11.appendSpan(doc, _el_0);
    import11.setAttribute(_el_1, 'j130Pede', '');
    this._J130Pede_1_5 = import1.J130Pede(import5.unsafeCast<ViewJ130Usa0>((this.parentView!))._J130B_0_6);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_1, this._J130Pede_1_5);
    }
    this.initRootNode(_el_0);
  }
}

import14.EmbeddedView<void> viewFactory_J130Usa1(import15.RenderView parentView, int parentIndex) {
  return _ViewJ130Usa1(parentView, parentIndex);
}

final List<Object> styles$J130UsaHost = const [];

class _ViewJ130UsaHost0 extends import8.HostView<import1.J130Usa> {
  @override
  void build() {
    this.componentView = ViewJ130Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J130Usa();
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J130Usa> viewFactory_J130UsaHost0() {
  return _ViewJ130UsaHost0();
}
