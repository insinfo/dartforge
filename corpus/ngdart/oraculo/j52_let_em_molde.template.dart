// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j52_let_em_molde.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j52_let_em_molde.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'j52_filho.template.dart' as import4;
import 'j52_filho.dart' as import5;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'dart:html' as import10;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/runtime/text_binding.dart' as import16;
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/views/render_view.dart' as import18;
import 'package:ngdart/src/runtime/interpolate.dart' as import19;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import20;

final List<Object> styles$J52LetEmMolde = const [];

class ViewJ52LetEmMolde0 extends import0.ComponentView<import1.J52LetEmMolde> {
  late final ViewContainer _appEl_1;
  late final TemplateRef _TemplateRef_1_7;
  late final import4.ViewJ52Filho0 _compView_2;
  late final import5.J52Filho _J52Filho_2_5;
  Object? _expr_0;
  static import6.ComponentStyles? _componentStyles;
  ViewJ52LetEmMolde0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import10.document.createElement('j52-let-em-molde'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/j52_let_em_molde.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import10.document;
    final _el_0 = import11.appendDiv(doc, parentRenderNode);
    final _anchor_1 = import11.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    this._TemplateRef_1_7 = TemplateRef(this._appEl_1, viewFactory_J52LetEmMolde1);
    this._compView_2 = import4.ViewJ52Filho0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    parentRenderNode.append(_el_2);
    this._J52Filho_2_5 = import5.J52Filho();
    this._compView_2.create(this._J52Filho_2_5);
  }

  @override
  void detectChangesInternal() {
    final local_modelo = this._TemplateRef_1_7;
    final currVal_0 = local_modelo;
    if (import12.checkBinding(this._expr_0, currVal_0, 'modelo', 'package:corpus_ngdart/src/j52_let_em_molde.html')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._J52Filho_2_5, 'modelo', currVal_0);
      }
      this._J52Filho_2_5.modelo = currVal_0 /* REF:package:corpus_ngdart/src/j52_let_em_molde.html:241:258 */;
      this._expr_0 = currVal_0;
    }
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_2.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$J52LetEmMolde, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J52LetEmMoldeNgFactory = ComponentFactory<import1.J52LetEmMolde>('j52-let-em-molde', viewFactory_J52LetEmMoldeHost0);
ComponentFactory<import1.J52LetEmMolde> get J52LetEmMoldeNgFactory {
  return _J52LetEmMoldeNgFactory;
}

ComponentFactory<import1.J52LetEmMolde> createJ52LetEmMoldeFactory() {
  return ComponentFactory('j52-let-em-molde', viewFactory_J52LetEmMoldeHost0);
}

class _ViewJ52LetEmMolde1 extends import15.EmbeddedView<import1.J52LetEmMolde> {
  final import16.TextBinding _textBinding_1 = import16.TextBinding();
  final import16.TextBinding _textBinding_4 = import16.TextBinding();
  late final ViewContainer _appEl_6;
  late final NgIf _NgIf_6_9;
  _ViewJ52LetEmMolde1(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import9.unsafeCast(doc.createElement('span'));
    _el_0.append(this._textBinding_1.element);
    final _text_2 = import11.createText(' ');
    final _el_3 = import9.unsafeCast(doc.createElement('small'));
    _el_3.append(this._textBinding_4.element);
    final _text_5 = import11.createText(' ');
    final _anchor_6 = import11.createAnchor();
    this._appEl_6 = ViewContainer(6, null, this, _anchor_6);
    var _TemplateRef_6_8 = TemplateRef(this._appEl_6, viewFactory_J52LetEmMolde2);
    this._NgIf_6_9 = NgIf(this._appEl_6, _TemplateRef_6_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_6, this._NgIf_6_9);
    }
    this.initRootNodesAndSubscriptions(import9.unsafeCast(<Object>[_el_0, _text_2, _el_3, _text_5, this._appEl_6]), null);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_ctx = this.locals['\$implicit'];
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_6_9, 'ngIf', local_ctx.anterior);
    }
    this._NgIf_6_9.ngIf = local_ctx.anterior /* REF:package:corpus_ngdart/src/j52_let_em_molde.html:118:138 */;
    this._appEl_6.detectChangesInNestedViews();
    this._textBinding_1.updateText(import19.interpolate0(local_ctx.titulo)) /* REF:package:corpus_ngdart/src/j52_let_em_molde.html:45:61 */;
    this._textBinding_4.updateText(import19.interpolateString0(_ctx.rotulo(local_ctx))) /* REF:package:corpus_ngdart/src/j52_let_em_molde.html:80:97 */;
  }

  @override
  void destroyInternal() {
    this._appEl_6.destroyNestedViews();
  }
}

import15.EmbeddedView<void> viewFactory_J52LetEmMolde1(import18.RenderView parentView, int parentIndex) {
  return _ViewJ52LetEmMolde1(parentView, parentIndex);
}

class _ViewJ52LetEmMolde2 extends import15.EmbeddedView<import1.J52LetEmMolde> {
  final import16.TextBinding _textBinding_1 = import16.TextBinding();
  Object? _expr_0;
  late final import10.ButtonElement _el_0;
  _ViewJ52LetEmMolde2(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    this._el_0 = import9.unsafeCast(doc.createElement('button'));
    this._el_0.append(this._textBinding_1.element);
    this._el_0.addEventListener('click', this.eventHandler1(this._handleEvent_0));
    this.initRootNode(this._el_0);
  }

  @override
  void detectChangesInternal() {
    final local_ctx = import9.unsafeCast<_ViewJ52LetEmMolde1>((this.parentView!)).locals['\$implicit'];
    final currVal_0 = local_ctx.classe;
    if (import12.checkBinding(this._expr_0, currVal_0, 'ctx.classe', 'package:corpus_ngdart/src/j52_let_em_molde.html')) {
      this.updateChildClass(this._el_0, currVal_0) /* REF:package:corpus_ngdart/src/j52_let_em_molde.html:139:159 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_1.updateText(import19.interpolate0(local_ctx.rotulo)) /* REF:package:corpus_ngdart/src/j52_let_em_molde.html:183:199 */;
  }

  void _handleEvent_0($event) {
    final local_ctx = import9.unsafeCast<_ViewJ52LetEmMolde1>((this.parentView!)).locals['\$implicit'];
    local_ctx.voltar();
  }
}

import15.EmbeddedView<void> viewFactory_J52LetEmMolde2(import18.RenderView parentView, int parentIndex) {
  return _ViewJ52LetEmMolde2(parentView, parentIndex);
}

final List<Object> styles$J52LetEmMoldeHost = const [];

class _ViewJ52LetEmMoldeHost0 extends import20.HostView<import1.J52LetEmMolde> {
  @override
  void build() {
    this.componentView = ViewJ52LetEmMolde0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J52LetEmMolde();
    this.initRootNode(_el_0);
  }
}

import20.HostView<import1.J52LetEmMolde> viewFactory_J52LetEmMoldeHost0() {
  return _ViewJ52LetEmMoldeHost0();
}
