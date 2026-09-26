// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i45_ref_formas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i45_ref_formas.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'dart:html' as import5;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/runtime/interpolate.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/common/directives/ng_for.dart' as import16;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;
import 'package:ngdart/src/runtime/check_binding.dart' as import18;
import 'a02_texto_estatico.template.dart' as import19;
import 'a02_texto_estatico.dart' as import20;
import 'dart:core';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import22;

final List<Object> styles$I45RefFormas = const [];

class ViewI45RefFormas0 extends import0.ComponentView<import1.I45RefFormas> {
  final import2.TextBinding _textBinding_2 = import2.TextBinding();
  late final ViewContainer _appEl_3;
  late final NgIf _NgIf_3_9;
  late final import5.InputElement _el_4;
  static import6.ComponentStyles? _componentStyles;
  ViewI45RefFormas0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import5.document.createElement('i45-ref-formas'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/i45_ref_formas.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import5.document;
    final _el_0 = import10.appendElement<import5.InputElement>(doc, parentRenderNode, 'input');
    final _el_1 = import10.appendElement<import5.HtmlElement>(doc, parentRenderNode, 'p');
    _el_1.append(this._textBinding_2.element);
    final _anchor_3 = import10.appendAnchor(parentRenderNode);
    this._appEl_3 = ViewContainer(3, null, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_I45RefFormas1);
    this._NgIf_3_9 = NgIf(this._appEl_3, _TemplateRef_3_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_3, this._NgIf_3_9);
    }
    this._el_4 = import10.appendElement<import5.InputElement>(doc, parentRenderNode, 'input');
    import10.setAttribute(this._el_4, 'type', 'checkbox');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_caixa = this._el_4;
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.recordInput(this._NgIf_3_9, 'ngIf', (local_caixa.checked == true));
    }
    this._NgIf_3_9.ngIf = (local_caixa.checked == true) /* REF:package:corpus_ngdart/src/i45_ref_formas.html:35:64 */;
    this._appEl_3.detectChangesInNestedViews();
    this._textBinding_2.updateText(import13.interpolateString0(_ctx.p.nome)) /* REF:package:corpus_ngdart/src/i45_ref_formas.html:16:26 */;
  }

  @override
  void destroyInternal() {
    this._appEl_3.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$I45RefFormas, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I45RefFormasNgFactory = ComponentFactory<import1.I45RefFormas>('i45-ref-formas', viewFactory_I45RefFormasHost0);
ComponentFactory<import1.I45RefFormas> get I45RefFormasNgFactory {
  return _I45RefFormasNgFactory;
}

ComponentFactory<import1.I45RefFormas> createI45RefFormasFactory() {
  return ComponentFactory('i45-ref-formas', viewFactory_I45RefFormasHost0);
}

class _ViewI45RefFormas1 extends import15.EmbeddedView<import1.I45RefFormas> {
  late final ViewContainer _appEl_3;
  late final import16.NgFor _NgFor_3_9;
  Object? _expr_0;
  late final import5.InputElement _el_1;
  _ViewI45RefFormas1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import5.document;
    final _el_0 = import9.unsafeCast(doc.createElement('div'));
    this._el_1 = import10.appendElement<import5.InputElement>(doc, _el_0, 'input');
    import10.setAttribute(this._el_1, 'type', 'checkbox');
    final _el_2 = import10.appendElement<import5.UListElement>(doc, _el_0, 'ul');
    final _anchor_3 = import10.appendAnchor(_el_2);
    this._appEl_3 = ViewContainer(3, 2, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_I45RefFormas2);
    this._NgFor_3_9 = import16.NgFor(this._appEl_3, _TemplateRef_3_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_3, this._NgFor_3_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.itens;
    if (import18.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/i45_ref_formas.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgFor_3_9, 'ngForOf', currVal_0);
      }
      this._NgFor_3_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i45_ref_formas.html:104:127 */;
      this._expr_0 = currVal_0;
    }
    if ((!import18.debugThrowIfChanged)) {
      this._NgFor_3_9.ngDoCheck();
    }
    this._appEl_3.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_3.destroyNestedViews();
  }
}

import15.EmbeddedView<void> viewFactory_I45RefFormas1(import17.RenderView parentView, int parentIndex) {
  return _ViewI45RefFormas1(parentView, parentIndex);
}

class _ViewI45RefFormas2 extends import15.EmbeddedView<import1.I45RefFormas> {
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  late final import19.ViewA02TextoEstatico0 _compView_1;
  late final import20.A02TextoEstatico _A02TextoEstatico_1_5;
  _ViewI45RefFormas2(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import5.document;
    final _el_0 = import9.unsafeCast(doc.createElement('li'));
    this._compView_1 = import19.ViewA02TextoEstatico0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    _el_0.append(_el_1);
    this._A02TextoEstatico_1_5 = import20.A02TextoEstatico();
    this._compView_1.create(this._A02TextoEstatico_1_5);
    final _el_2 = import10.appendElement<import5.HtmlElement>(doc, _el_0, 'b');
    _el_2.append(this._textBinding_3.element);
    _el_2.addEventListener('click', this.eventHandler1(this._handleEvent_0));
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_x = import9.unsafeCast<String>(this.locals['\$implicit']);
    this._textBinding_3.updateText(import13.interpolateString0(local_x)) /* REF:package:corpus_ngdart/src/i45_ref_formas.html:230:235 */;
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_1.destroyInternalState();
  }

  void _handleEvent_0($event) {
    final local_filho = this._A02TextoEstatico_1_5;
    final local_caixa2 = import9.unsafeCast<_ViewI45RefFormas1>((this.parentView!))._el_1;
    final local_caixa = import9.unsafeCast<ViewI45RefFormas0>(((this.parentView!).parentView!))._el_4;
    final _ctx = this.ctx;
    _ctx.usar(local_filho, local_caixa2.checked, local_caixa.value);
  }
}

import15.EmbeddedView<void> viewFactory_I45RefFormas2(import17.RenderView parentView, int parentIndex) {
  return _ViewI45RefFormas2(parentView, parentIndex);
}

final List<Object> styles$I45RefFormasHost = const [];

class _ViewI45RefFormasHost0 extends import22.HostView<import1.I45RefFormas> {
  @override
  void build() {
    this.componentView = ViewI45RefFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I45RefFormas();
    this.initRootNode(_el_0);
  }
}

import22.HostView<import1.I45RefFormas> viewFactory_I45RefFormasHost0() {
  return _ViewI45RefFormasHost0();
}
