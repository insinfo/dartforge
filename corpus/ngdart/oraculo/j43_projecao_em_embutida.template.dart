// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j43_projecao_em_embutida.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j43_projecao_em_embutida.dart' as import1;
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

final List<Object> styles$J43ProjecaoEmEmbutida = const [];

class ViewJ43ProjecaoEmEmbutida0 extends import0.ComponentView<import1.J43ProjecaoEmEmbutida> {
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  static import4.ComponentStyles? _componentStyles;
  ViewJ43ProjecaoEmEmbutida0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j43-projecao-em-embutida'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j43_projecao_em_embutida.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendElement<import8.HtmlElement>(doc, parentRenderNode, 'header');
    this.project(_el_0, 0);
    final _anchor_1 = import9.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J43ProjecaoEmEmbutida1);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
    this.project(parentRenderNode, 3);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.a);
    }
    this._NgIf_1_9.ngIf = _ctx.a /* REF:package:corpus_ngdart/src/j43_projecao_em_embutida.html:64:73 */;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J43ProjecaoEmEmbutida, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J43ProjecaoEmEmbutidaNgFactory = ComponentFactory<import1.J43ProjecaoEmEmbutida>('j43-projecao-em-embutida', viewFactory_J43ProjecaoEmEmbutidaHost0);
ComponentFactory<import1.J43ProjecaoEmEmbutida> get J43ProjecaoEmEmbutidaNgFactory {
  return _J43ProjecaoEmEmbutidaNgFactory;
}

ComponentFactory<import1.J43ProjecaoEmEmbutida> createJ43ProjecaoEmEmbutidaFactory() {
  return ComponentFactory('j43-projecao-em-embutida', viewFactory_J43ProjecaoEmEmbutidaHost0);
}

class _ViewJ43ProjecaoEmEmbutida1 extends import13.EmbeddedView<import1.J43ProjecaoEmEmbutida> {
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  _ViewJ43ProjecaoEmEmbutida1(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    this.project(_el_0, 1);
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J43ProjecaoEmEmbutida2);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.b);
    }
    this._NgIf_1_9.ngIf = _ctx.b /* REF:package:corpus_ngdart/src/j43_projecao_em_embutida.html:118:127 */;
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import13.EmbeddedView<void> viewFactory_J43ProjecaoEmEmbutida1(import14.RenderView parentView, int parentIndex) {
  return _ViewJ43ProjecaoEmEmbutida1(parentView, parentIndex);
}

class _ViewJ43ProjecaoEmEmbutida2 extends import13.EmbeddedView<import1.J43ProjecaoEmEmbutida> {
  _ViewJ43ProjecaoEmEmbutida2(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    this.project(_el_0, 2);
    this.initRootNode(_el_0);
  }
}

import13.EmbeddedView<void> viewFactory_J43ProjecaoEmEmbutida2(import14.RenderView parentView, int parentIndex) {
  return _ViewJ43ProjecaoEmEmbutida2(parentView, parentIndex);
}

final List<Object> styles$J43ProjecaoEmEmbutidaHost = const [];

class _ViewJ43ProjecaoEmEmbutidaHost0 extends import15.HostView<import1.J43ProjecaoEmEmbutida> {
  @override
  void build() {
    this.componentView = ViewJ43ProjecaoEmEmbutida0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J43ProjecaoEmEmbutida();
    this.initRootNode(_el_0);
  }
}

import15.HostView<import1.J43ProjecaoEmEmbutida> viewFactory_J43ProjecaoEmEmbutidaHost0() {
  return _ViewJ43ProjecaoEmEmbutidaHost0();
}
