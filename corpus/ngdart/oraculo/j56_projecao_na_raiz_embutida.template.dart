// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j56_projecao_na_raiz_embutida.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j56_projecao_na_raiz_embutida.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import13;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import15;

final List<Object> styles$J56ProjecaoNaRaizEmbutida = const [];

class ViewJ56ProjecaoNaRaizEmbutida0 extends import0.ComponentView<import1.J56ProjecaoNaRaizEmbutida> {
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  late final ViewContainer _appEl_3;
  late final NgIf _NgIf_3_9;
  static import4.ComponentStyles? _componentStyles;
  ViewJ56ProjecaoNaRaizEmbutida0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j56-projecao-na-raiz-embutida'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j56_projecao_na_raiz_embutida.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J56ProjecaoNaRaizEmbutida1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
    final _text_1 = import9.appendText(parentRenderNode, '\n');
    final _anchor_2 = import9.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J56ProjecaoNaRaizEmbutida2);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    final _anchor_3 = import9.appendAnchor(parentRenderNode);
    this._appEl_3 = ViewContainer(3, null, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_J56ProjecaoNaRaizEmbutida3);
    this._NgIf_3_9 = NgIf(this._appEl_3, _TemplateRef_3_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_3, this._NgIf_3_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.a);
    }
    this._NgIf_0_9.ngIf = _ctx.a /* REF:package:corpus_ngdart/src/j56_projecao_na_raiz_embutida.html:10:20 */;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.b);
    }
    this._NgIf_2_9.ngIf = _ctx.b /* REF:package:corpus_ngdart/src/j56_projecao_na_raiz_embutida.html:92:101 */;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_3_9, 'ngIf', _ctx.c);
    }
    this._NgIf_3_9.ngIf = _ctx.c /* REF:package:corpus_ngdart/src/j56_projecao_na_raiz_embutida.html:177:186 */;
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_2.detectChangesInNestedViews();
    this._appEl_3.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this._appEl_2.destroyNestedViews();
    this._appEl_3.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J56ProjecaoNaRaizEmbutida, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J56ProjecaoNaRaizEmbutidaNgFactory = ComponentFactory<import1.J56ProjecaoNaRaizEmbutida>('j56-projecao-na-raiz-embutida', viewFactory_J56ProjecaoNaRaizEmbutidaHost0);
ComponentFactory<import1.J56ProjecaoNaRaizEmbutida> get J56ProjecaoNaRaizEmbutidaNgFactory {
  return _J56ProjecaoNaRaizEmbutidaNgFactory;
}

ComponentFactory<import1.J56ProjecaoNaRaizEmbutida> createJ56ProjecaoNaRaizEmbutidaFactory() {
  return ComponentFactory('j56-projecao-na-raiz-embutida', viewFactory_J56ProjecaoNaRaizEmbutidaHost0);
}

class _ViewJ56ProjecaoNaRaizEmbutida1 extends import13.EmbeddedView<import1.J56ProjecaoNaRaizEmbutida> {
  _ViewJ56ProjecaoNaRaizEmbutida1(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this.initRootNodesAndSubscriptions(import7.unsafeCast(this.projectedNodes[0]), null);
  }
}

import13.EmbeddedView<void> viewFactory_J56ProjecaoNaRaizEmbutida1(import14.RenderView parentView, int parentIndex) {
  return _ViewJ56ProjecaoNaRaizEmbutida1(parentView, parentIndex);
}

class _ViewJ56ProjecaoNaRaizEmbutida2 extends import13.EmbeddedView<import1.J56ProjecaoNaRaizEmbutida> {
  _ViewJ56ProjecaoNaRaizEmbutida2(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('i'));
    final _text_1 = import9.appendText(_el_0, 'depois');
    this.initRootNodesAndSubscriptions(import7.unsafeCast(<Object>[this.projectedNodes[1]]..addAll(<Object>[_el_0])), null);
  }
}

import13.EmbeddedView<void> viewFactory_J56ProjecaoNaRaizEmbutida2(import14.RenderView parentView, int parentIndex) {
  return _ViewJ56ProjecaoNaRaizEmbutida2(parentView, parentIndex);
}

class _ViewJ56ProjecaoNaRaizEmbutida3 extends import13.EmbeddedView<import1.J56ProjecaoNaRaizEmbutida> {
  _ViewJ56ProjecaoNaRaizEmbutida3(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    this.project(_el_0, 2);
    this.initRootNode(_el_0);
  }
}

import13.EmbeddedView<void> viewFactory_J56ProjecaoNaRaizEmbutida3(import14.RenderView parentView, int parentIndex) {
  return _ViewJ56ProjecaoNaRaizEmbutida3(parentView, parentIndex);
}

final List<Object> styles$J56ProjecaoNaRaizEmbutidaHost = const [];

class _ViewJ56ProjecaoNaRaizEmbutidaHost0 extends import15.HostView<import1.J56ProjecaoNaRaizEmbutida> {
  @override
  void build() {
    this.componentView = ViewJ56ProjecaoNaRaizEmbutida0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J56ProjecaoNaRaizEmbutida();
    this.initRootNode(_el_0);
  }
}

import15.HostView<import1.J56ProjecaoNaRaizEmbutida> viewFactory_J56ProjecaoNaRaizEmbutidaHost0() {
  return _ViewJ56ProjecaoNaRaizEmbutidaHost0();
}
