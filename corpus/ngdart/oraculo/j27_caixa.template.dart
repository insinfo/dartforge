// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j27_caixa.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j27_caixa.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_template_outlet.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import16;

final List<Object> styles$J27Caixa = const [];

class ViewJ27Caixa0 extends import0.ComponentView<import1.J27Caixa> {
  late final ViewContainer _appEl_1;
  late final import3.NgTemplateOutlet _NgTemplateOutlet_1_9;
  Object? _expr_0;
  Object? _expr_1;
  static import4.ComponentStyles? _componentStyles;
  ViewJ27Caixa0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j27-caixa'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j27_caixa.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendDiv(doc, parentRenderNode);
    this.updateChildClass(_el_0, 'gatilho');
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J27Caixa1);
    this._NgTemplateOutlet_1_9 = import3.NgTemplateOutlet(this._appEl_1);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgTemplateOutlet_1_9);
    }
    this.project(parentRenderNode, 0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.gatilho?.templateRef;
    if (import12.checkBinding(this._expr_0, currVal_0, 'gatilho?.templateRef', 'package:corpus_ngdart/src/j27_caixa.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgTemplateOutlet_1_9, 'ngTemplateOutlet', currVal_0);
      }
      this._NgTemplateOutlet_1_9.ngTemplateOutlet = currVal_0 /* REF:package:corpus_ngdart/src/j27_caixa.html:34:75 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.contexto;
    if (import12.checkBinding(this._expr_1, currVal_1, 'contexto', 'package:corpus_ngdart/src/j27_caixa.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgTemplateOutlet_1_9, 'ngTemplateOutletValue', currVal_1);
      }
      this._NgTemplateOutlet_1_9.ngTemplateOutletValue = currVal_1 /* REF:package:corpus_ngdart/src/j27_caixa.html:76:110 */;
      this._expr_1 = currVal_1;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgTemplateOutlet_1_9.ngDoCheck();
    }
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J27Caixa, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J27CaixaNgFactory = ComponentFactory<import1.J27Caixa>('j27-caixa', viewFactory_J27CaixaHost0);
ComponentFactory<import1.J27Caixa> get J27CaixaNgFactory {
  return _J27CaixaNgFactory;
}

ComponentFactory<import1.J27Caixa> createJ27CaixaFactory() {
  return ComponentFactory('j27-caixa', viewFactory_J27CaixaHost0);
}

class _ViewJ27Caixa1 extends import14.EmbeddedView<import1.J27Caixa> {
  _ViewJ27Caixa1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this.initRootNodesAndSubscriptions(import7.unsafeCast(const <Object>[]), null);
  }
}

import14.EmbeddedView<void> viewFactory_J27Caixa1(import15.RenderView parentView, int parentIndex) {
  return _ViewJ27Caixa1(parentView, parentIndex);
}

final List<Object> styles$J27CaixaHost = const [];

class _ViewJ27CaixaHost0 extends import16.HostView<import1.J27Caixa> {
  @override
  void build() {
    this.componentView = ViewJ27Caixa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J27Caixa();
    this.initRootNode(_el_0);
  }
}

import16.HostView<import1.J27Caixa> viewFactory_J27CaixaHost0() {
  return _ViewJ27CaixaHost0();
}
