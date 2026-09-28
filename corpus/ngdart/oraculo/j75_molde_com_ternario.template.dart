// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j75_molde_com_ternario.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j75_molde_com_ternario.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_template_outlet.dart' as import3;
import 'package:ngdart/src/common/directives/ng_for.dart' as import4;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'dart:html' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/runtime/check_binding.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/runtime/text_binding.dart' as import17;
import 'dart:core';
import 'package:ngdart/src/runtime/interpolate.dart' as import19;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import20;

final List<Object> styles$J75MoldeComTernario = const [];

class ViewJ75MoldeComTernario0 extends import0.ComponentView<import1.J75MoldeComTernario> {
  late final ViewContainer _appEl_0;
  late final import3.NgTemplateOutlet _NgTemplateOutlet_0_9;
  late final ViewContainer _appEl_1;
  late final import4.NgFor _NgFor_1_9;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  static import5.ComponentStyles? _componentStyles;
  ViewJ75MoldeComTernario0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('j75-molde-com-ternario'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/j75_molde_com_ternario.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import10.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J75MoldeComTernario1);
    this._NgTemplateOutlet_0_9 = import3.NgTemplateOutlet(this._appEl_0);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_0, this._NgTemplateOutlet_0_9);
    }
    final _anchor_1 = import10.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J75MoldeComTernario2);
    this._NgFor_1_9 = import4.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = ((_ctx.modelo == null) ? null : _ctx.outro);
    if (import13.checkBinding(this._expr_0, currVal_0, 'modelo == null ? null : outro', 'package:corpus_ngdart/src/j75_molde_com_ternario.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgTemplateOutlet_0_9, 'ngTemplateOutlet', currVal_0);
      }
      this._NgTemplateOutlet_0_9.ngTemplateOutlet = currVal_0 /* REF:package:corpus_ngdart/src/j75_molde_com_ternario.html:10:60 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.contexto;
    if (import13.checkBinding(this._expr_1, currVal_1, 'contexto', 'package:corpus_ngdart/src/j75_molde_com_ternario.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgTemplateOutlet_0_9, 'ngTemplateOutletValue', currVal_1);
      }
      this._NgTemplateOutlet_0_9.ngTemplateOutletValue = currVal_1 /* REF:package:corpus_ngdart/src/j75_molde_com_ternario.html:61:95 */;
      this._expr_1 = currVal_1;
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgTemplateOutlet_0_9.ngDoCheck();
    }
    final currVal_2 = _ctx.itens;
    if (import13.checkBinding(this._expr_2, currVal_2, 'itens', 'package:corpus_ngdart/src/j75_molde_com_ternario.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_2);
      }
      this._NgFor_1_9.ngForOf = currVal_2 /* REF:package:corpus_ngdart/src/j75_molde_com_ternario.html:111:149 */;
      this._expr_2 = currVal_2;
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgFor_1_9.ngDoCheck();
    }
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this._appEl_1.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$J75MoldeComTernario, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J75MoldeComTernarioNgFactory = ComponentFactory<import1.J75MoldeComTernario>('j75-molde-com-ternario', viewFactory_J75MoldeComTernarioHost0);
ComponentFactory<import1.J75MoldeComTernario> get J75MoldeComTernarioNgFactory {
  return _J75MoldeComTernarioNgFactory;
}

ComponentFactory<import1.J75MoldeComTernario> createJ75MoldeComTernarioFactory() {
  return ComponentFactory('j75-molde-com-ternario', viewFactory_J75MoldeComTernarioHost0);
}

class _ViewJ75MoldeComTernario1 extends import15.EmbeddedView<import1.J75MoldeComTernario> {
  _ViewJ75MoldeComTernario1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this.initRootNodesAndSubscriptions(import8.unsafeCast(const <Object>[]), null);
  }
}

import15.EmbeddedView<void> viewFactory_J75MoldeComTernario1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ75MoldeComTernario1(parentView, parentIndex);
}

class _ViewJ75MoldeComTernario2 extends import15.EmbeddedView<import1.J75MoldeComTernario> {
  final import17.TextBinding _textBinding_1 = import17.TextBinding();
  _ViewJ75MoldeComTernario2(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('p'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_x = import8.unsafeCast<String>(this.locals['\$implicit']);
    final local_i = import8.unsafeCast<int>(this.locals['index']);
    this._textBinding_1.updateText(import19.interpolateString0(_ctx.rotulo(((local_x + ';') + local_i.toString())))) /* REF:package:corpus_ngdart/src/j75_molde_com_ternario.html:150:186 */;
  }
}

import15.EmbeddedView<void> viewFactory_J75MoldeComTernario2(import16.RenderView parentView, int parentIndex) {
  return _ViewJ75MoldeComTernario2(parentView, parentIndex);
}

final List<Object> styles$J75MoldeComTernarioHost = const [];

class _ViewJ75MoldeComTernarioHost0 extends import20.HostView<import1.J75MoldeComTernario> {
  @override
  void build() {
    this.componentView = ViewJ75MoldeComTernario0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J75MoldeComTernario();
    this.initRootNode(_el_0);
  }
}

import20.HostView<import1.J75MoldeComTernario> viewFactory_J75MoldeComTernarioHost0() {
  return _ViewJ75MoldeComTernarioHost0();
}
