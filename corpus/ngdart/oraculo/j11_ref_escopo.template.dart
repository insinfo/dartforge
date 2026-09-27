// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j11_ref_escopo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j11_ref_escopo.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import7;
import 'package:ngdart/src/core/linker/views/view.dart' as import8;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import9;
import 'package:ngdart/src/utilities.dart' as import10;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/src/runtime/check_binding.dart' as import14;
import 'package:ngdart/src/runtime/interpolate.dart' as import15;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import17;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import18;
import 'dart:core';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import20;

final List<Object> styles$J11RefEscopo = const [];

class ViewJ11RefEscopo0 extends import0.ComponentView<import1.J11RefEscopo> {
  final import2.TextBinding _textBinding_2 = import2.TextBinding();
  late final ViewContainer _appEl_3;
  late final NgIf _NgIf_3_9;
  late final ViewContainer _appEl_4;
  late final NgIf _NgIf_4_9;
  late final ViewContainer _appEl_6;
  late final import5.NgFor _NgFor_6_9;
  late final import6.HtmlElement _el_1;
  Object? _expr_0;
  late final import6.InputElement _el_0;
  static import7.ComponentStyles? _componentStyles;
  ViewJ11RefEscopo0(import8.View parentView, int parentIndex) : super(parentView, parentIndex, import9.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import10.unsafeCast(import6.document.createElement('j11-ref-escopo'));
  }
  static String? get _debugComponentUrl {
    return (import10.isDevMode ? 'asset:corpus_ngdart/lib/src/j11_ref_escopo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    this._el_0 = import11.appendElement<import6.InputElement>(doc, parentRenderNode, 'input');
    import11.setAttribute(this._el_0, 'value', 'fora');
    this._el_1 = import11.appendSpan(doc, parentRenderNode);
    this._el_1.append(this._textBinding_2.element);
    final _anchor_3 = import11.appendAnchor(parentRenderNode);
    this._appEl_3 = ViewContainer(3, null, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_J11RefEscopo1);
    this._NgIf_3_9 = NgIf(this._appEl_3, _TemplateRef_3_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_3, this._NgIf_3_9);
    }
    final _anchor_4 = import11.appendAnchor(parentRenderNode);
    this._appEl_4 = ViewContainer(4, null, this, _anchor_4);
    var _TemplateRef_4_8 = TemplateRef(this._appEl_4, viewFactory_J11RefEscopo3);
    this._NgIf_4_9 = NgIf(this._appEl_4, _TemplateRef_4_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_4, this._NgIf_4_9);
    }
    final _el_5 = import11.appendElement<import6.UListElement>(doc, parentRenderNode, 'ul');
    final _anchor_6 = import11.appendAnchor(_el_5);
    this._appEl_6 = ViewContainer(6, 5, this, _anchor_6);
    var _TemplateRef_6_8 = TemplateRef(this._appEl_6, viewFactory_J11RefEscopo4);
    this._NgFor_6_9 = import5.NgFor(this._appEl_6, _TemplateRef_6_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_6, this._NgFor_6_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_n = this._el_0;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_3_9, 'ngIf', _ctx.a);
    }
    this._NgIf_3_9.ngIf = _ctx.a /* REF:package:corpus_ngdart/src/j11_ref_escopo.html:55:64 */;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_4_9, 'ngIf', _ctx.b);
    }
    this._NgIf_4_9.ngIf = _ctx.b /* REF:package:corpus_ngdart/src/j11_ref_escopo.html:174:183 */;
    final currVal_0 = _ctx.itens;
    if (import14.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/j11_ref_escopo.html')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._NgFor_6_9, 'ngForOf', currVal_0);
      }
      this._NgFor_6_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/j11_ref_escopo.html:221:244 */;
      this._expr_0 = currVal_0;
    }
    if ((!import14.debugThrowIfChanged)) {
      this._NgFor_6_9.ngDoCheck();
    }
    this._appEl_3.detectChangesInNestedViews();
    this._appEl_4.detectChangesInNestedViews();
    this._appEl_6.detectChangesInNestedViews();
    this._textBinding_2.updateText(import15.interpolate0(local_n.value)) /* REF:package:corpus_ngdart/src/j11_ref_escopo.html:32:43 */;
  }

  @override
  void destroyInternal() {
    this._appEl_3.destroyNestedViews();
    this._appEl_4.destroyNestedViews();
    this._appEl_6.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import7.ComponentStyles.unscoped(styles$J11RefEscopo, _debugComponentUrl));
      if (import10.isDevMode) {
        import7.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J11RefEscopoNgFactory = ComponentFactory<import1.J11RefEscopo>('j11-ref-escopo', viewFactory_J11RefEscopoHost0);
ComponentFactory<import1.J11RefEscopo> get J11RefEscopoNgFactory {
  return _J11RefEscopoNgFactory;
}

ComponentFactory<import1.J11RefEscopo> createJ11RefEscopoFactory() {
  return ComponentFactory('j11-ref-escopo', viewFactory_J11RefEscopoHost0);
}

class _ViewJ11RefEscopo1 extends import17.EmbeddedView<import1.J11RefEscopo> {
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  _ViewJ11RefEscopo1(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import10.unsafeCast(doc.createElement('div'));
    final _el_1 = import11.appendElement<import6.InputElement>(doc, _el_0, 'input');
    import11.setAttribute(_el_1, 'value', 'meio');
    final _anchor_2 = import11.appendAnchor(_el_0);
    this._appEl_2 = ViewContainer(2, 0, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J11RefEscopo2);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.b);
    }
    this._NgIf_2_9.ngIf = _ctx.b /* REF:package:corpus_ngdart/src/j11_ref_escopo.html:91:100 */;
    this._appEl_2.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_2.destroyNestedViews();
  }
}

import17.EmbeddedView<void> viewFactory_J11RefEscopo1(import18.RenderView parentView, int parentIndex) {
  return _ViewJ11RefEscopo1(parentView, parentIndex);
}

class _ViewJ11RefEscopo2 extends import17.EmbeddedView<import1.J11RefEscopo> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  final import2.TextBinding _textBinding_5 = import2.TextBinding();
  late final import6.InputElement _el_4;
  _ViewJ11RefEscopo2(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import10.unsafeCast(doc.createElement('p'));
    _el_0.append(this._textBinding_1.element);
    final _text_2 = import11.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_3.element);
    this._el_4 = import11.appendElement<import6.InputElement>(doc, _el_0, 'input');
    import11.setAttribute(this._el_4, 'value', 'dentro');
    _el_0.append(this._textBinding_5.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_n = this._el_4;
    final local_s = import10.unsafeCast<ViewJ11RefEscopo0>(((this.parentView!).parentView!))._el_1;
    this._textBinding_1.updateText(import15.interpolate0(local_n.value)) /* REF:package:corpus_ngdart/src/j11_ref_escopo.html:101:112 */;
    this._textBinding_3.updateText(import15.interpolate0(local_s.text)) /* REF:package:corpus_ngdart/src/j11_ref_escopo.html:113:123 */;
    this._textBinding_5.updateText(import15.interpolate0(local_n.value)) /* REF:package:corpus_ngdart/src/j11_ref_escopo.html:148:159 */;
  }
}

import17.EmbeddedView<void> viewFactory_J11RefEscopo2(import18.RenderView parentView, int parentIndex) {
  return _ViewJ11RefEscopo2(parentView, parentIndex);
}

class _ViewJ11RefEscopo3 extends import17.EmbeddedView<import1.J11RefEscopo> {
  final import2.TextBinding _textBinding_2 = import2.TextBinding();
  late final import6.InputElement _el_1;
  _ViewJ11RefEscopo3(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import10.unsafeCast(doc.createElement('div'));
    this._el_1 = import11.appendElement<import6.InputElement>(doc, _el_0, 'input');
    _el_0.append(this._textBinding_2.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_so = this._el_1;
    this._textBinding_2.updateText(import15.interpolate0(local_so.value)) /* REF:package:corpus_ngdart/src/j11_ref_escopo.html:195:207 */;
  }
}

import17.EmbeddedView<void> viewFactory_J11RefEscopo3(import18.RenderView parentView, int parentIndex) {
  return _ViewJ11RefEscopo3(parentView, parentIndex);
}

class _ViewJ11RefEscopo4 extends import17.EmbeddedView<import1.J11RefEscopo> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  _ViewJ11RefEscopo4(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import10.unsafeCast(doc.createElement('li'));
    _el_0.append(this._textBinding_1.element);
    _el_0.addEventListener('click', this.eventHandler1(this._handleEvent_0));
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_n = import10.unsafeCast<String>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import15.interpolateString0(local_n)) /* REF:package:corpus_ngdart/src/j11_ref_escopo.html:262:267 */;
  }

  void _handleEvent_0($event) {
    final local_n = import10.unsafeCast<String>(this.locals['\$implicit']);
    final _ctx = this.ctx;
    _ctx.ver(local_n);
  }
}

import17.EmbeddedView<void> viewFactory_J11RefEscopo4(import18.RenderView parentView, int parentIndex) {
  return _ViewJ11RefEscopo4(parentView, parentIndex);
}

final List<Object> styles$J11RefEscopoHost = const [];

class _ViewJ11RefEscopoHost0 extends import20.HostView<import1.J11RefEscopo> {
  @override
  void build() {
    this.componentView = ViewJ11RefEscopo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J11RefEscopo();
    this.initRootNode(_el_0);
  }
}

import20.HostView<import1.J11RefEscopo> viewFactory_J11RefEscopoHost0() {
  return _ViewJ11RefEscopoHost0();
}
