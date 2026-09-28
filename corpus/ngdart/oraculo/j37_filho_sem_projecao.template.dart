// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j37_filho_sem_projecao.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j37_filho_sem_projecao.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/runtime/text_binding.dart' as import10;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import14;
import 'package:ngdart/src/runtime/check_binding.dart' as import15;
import 'package:ngdart/src/runtime/interpolate.dart' as import16;
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import17;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import18;

final List<Object> styles$J37SemSlot = const [];

class ViewJ37SemSlot0 extends import0.ComponentView<import1.J37SemSlot> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ37SemSlot0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j37-sem-slot'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j37_filho_sem_projecao.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'b');
    final _text_1 = import7.appendText(_el_0, 'fixo');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J37SemSlot, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J37SemSlotNgFactory = ComponentFactory<import1.J37SemSlot>('j37-sem-slot', viewFactory_J37SemSlotHost0);
ComponentFactory<import1.J37SemSlot> get J37SemSlotNgFactory {
  return _J37SemSlotNgFactory;
}

ComponentFactory<import1.J37SemSlot> createJ37SemSlotFactory() {
  return ComponentFactory('j37-sem-slot', viewFactory_J37SemSlotHost0);
}

final List<Object> styles$J37SemSlotHost = const [];

class _ViewJ37SemSlotHost0 extends import9.HostView<import1.J37SemSlot> {
  @override
  void build() {
    this.componentView = ViewJ37SemSlot0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J37SemSlot();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J37SemSlot> viewFactory_J37SemSlotHost0() {
  return _ViewJ37SemSlotHost0();
}

final List<Object> styles$J37ComSlot = const [];

class ViewJ37ComSlot0 extends import0.ComponentView<import1.J37ComSlot> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ37ComSlot0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j37-com-slot'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j37_filho_sem_projecao.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    this.project(_el_0, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J37ComSlot, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J37ComSlotNgFactory = ComponentFactory<import1.J37ComSlot>('j37-com-slot', viewFactory_J37ComSlotHost0);
ComponentFactory<import1.J37ComSlot> get J37ComSlotNgFactory {
  return _J37ComSlotNgFactory;
}

ComponentFactory<import1.J37ComSlot> createJ37ComSlotFactory() {
  return ComponentFactory('j37-com-slot', viewFactory_J37ComSlotHost0);
}

final List<Object> styles$J37ComSlotHost = const [];

class _ViewJ37ComSlotHost0 extends import9.HostView<import1.J37ComSlot> {
  @override
  void build() {
    this.componentView = ViewJ37ComSlot0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J37ComSlot();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J37ComSlot> viewFactory_J37ComSlotHost0() {
  return _ViewJ37ComSlotHost0();
}

final List<Object> styles$J37SoB = const [];

class ViewJ37SoB0 extends import0.ComponentView<import1.J37SoB> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ37SoB0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j37-so-b'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j37_filho_sem_projecao.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'p');
    this.project(_el_0, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J37SoB, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J37SoBNgFactory = ComponentFactory<import1.J37SoB>('j37-so-b', viewFactory_J37SoBHost0);
ComponentFactory<import1.J37SoB> get J37SoBNgFactory {
  return _J37SoBNgFactory;
}

ComponentFactory<import1.J37SoB> createJ37SoBFactory() {
  return ComponentFactory('j37-so-b', viewFactory_J37SoBHost0);
}

final List<Object> styles$J37SoBHost = const [];

class _ViewJ37SoBHost0 extends import9.HostView<import1.J37SoB> {
  @override
  void build() {
    this.componentView = ViewJ37SoB0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J37SoB();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J37SoB> viewFactory_J37SoBHost0() {
  return _ViewJ37SoBHost0();
}

final List<Object> styles$J37FilhoSemProjecao = const [];

class ViewJ37FilhoSemProjecao0 extends import0.ComponentView<import1.J37FilhoSemProjecao> {
  final import10.TextBinding _textBinding_12 = import10.TextBinding();
  late final ViewJ37SemSlot0 _compView_0;
  late final import1.J37SemSlot _J37SemSlot_0_5;
  late final ViewJ37SemSlot0 _compView_8;
  late final import1.J37SemSlot _J37SemSlot_8_5;
  late final ViewJ37ComSlot0 _compView_10;
  late final import1.J37ComSlot _J37ComSlot_10_5;
  late final ViewJ37SoB0 _compView_17;
  late final import1.J37SoB _J37SoB_17_5;
  late final ViewContainer _appEl_24;
  late final NgIf _NgIf_24_9;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  late final import6.HtmlElement _el_4;
  late final import6.HtmlElement _el_22;
  static import2.ComponentStyles? _componentStyles;
  ViewJ37FilhoSemProjecao0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j37-filho-sem-projecao'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j37_filho_sem_projecao.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ37SemSlot0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J37SemSlot_0_5 = import1.J37SemSlot();
    final doc = import6.document;
    this._el_4 = import5.unsafeCast(doc.createElement('span'));
    final _text_5 = import7.appendText(this._el_4, 'x');
    final _el_6 = import5.unsafeCast(doc.createElement('i'));
    final _text_7 = import7.appendText(_el_6, 'y');
    this._compView_0.create(this._J37SemSlot_0_5);
    this._compView_8 = ViewJ37SemSlot0(this, 8);
    final _el_8 = this._compView_8.rootElement;
    parentRenderNode.append(_el_8);
    this._J37SemSlot_8_5 = import1.J37SemSlot();
    this._compView_8.create(this._J37SemSlot_8_5);
    this._compView_10 = ViewJ37ComSlot0(this, 10);
    final _el_10 = this._compView_10.rootElement;
    parentRenderNode.append(_el_10);
    this._J37ComSlot_10_5 = import1.J37ComSlot();
    final _text_11 = import7.createText('ola ');
    final _text_13 = import7.createText('!');
    final _el_14 = import5.unsafeCast(doc.createElement('b'));
    final _text_15 = import7.appendText(_el_14, 'n');
    final _text_16 = import7.createText('fixo');
    this._compView_10.createAndProject(this._J37ComSlot_10_5, [
      <Object>[_text_11, this._textBinding_12.element, _text_13, _el_14, _text_16]
    ]);
    this._compView_17 = ViewJ37SoB0(this, 17);
    final _el_17 = this._compView_17.rootElement;
    parentRenderNode.append(_el_17);
    this._J37SoB_17_5 = import1.J37SoB();
    final _el_20 = import5.unsafeCast(doc.createElement('b'));
    final _text_21 = import7.appendText(_el_20, 'dentro');
    this._el_22 = import5.unsafeCast(doc.createElement('i'));
    final _text_23 = import7.appendText(this._el_22, 'z');
    final _anchor_24 = import7.createAnchor();
    this._appEl_24 = ViewContainer(24, 17, this, _anchor_24);
    var _TemplateRef_24_8 = TemplateRef(this._appEl_24, viewFactory_J37FilhoSemProjecao1);
    this._NgIf_24_9 = NgIf(this._appEl_24, _TemplateRef_24_8);
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_anchor_24, this._NgIf_24_9);
    }
    this._compView_17.createAndProject(this._J37SoB_17_5, [
      <Object>[_el_20]
    ]);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.nome;
    if (import15.checkBinding(this._expr_0, currVal_0, 'nome', 'package:corpus_ngdart/src/j37_filho_sem_projecao.html')) {
      if (import14.isDevToolsEnabled) {
        import14.Inspector.instance.recordInput(this._J37SemSlot_0_5, 'titulo', currVal_0);
      }
      this._J37SemSlot_0_5.titulo = currVal_0 /* REF:package:corpus_ngdart/src/j37_filho_sem_projecao.html:14:29 */;
      this._expr_0 = currVal_0;
    }
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.recordInput(this._NgIf_24_9, 'ngIf', _ctx.nome.isEmpty);
    }
    this._NgIf_24_9.ngIf = _ctx.nome.isEmpty /* REF:package:corpus_ngdart/src/j37_filho_sem_projecao.html:260:280 */;
    this._appEl_24.detectChangesInNestedViews();
    final currVal_1 = _ctx.nome;
    if (import15.checkBinding(this._expr_1, currVal_1, 'nome', 'package:corpus_ngdart/src/j37_filho_sem_projecao.html')) {
      import7.setProperty(this._el_4, 'title', currVal_1) /* REF:package:corpus_ngdart/src/j37_filho_sem_projecao.html:51:65 */;
      this._expr_1 = currVal_1;
    }
    this._textBinding_12.updateText(import16.interpolateString0(_ctx.nome)) /* REF:package:corpus_ngdart/src/j37_filho_sem_projecao.html:154:162 */;
    final currVal_2 = _ctx.nome;
    if (import15.checkBinding(this._expr_2, currVal_2, 'nome', 'package:corpus_ngdart/src/j37_filho_sem_projecao.html')) {
      import7.setProperty(this._el_22, 'title', currVal_2) /* REF:package:corpus_ngdart/src/j37_filho_sem_projecao.html:236:250 */;
      this._expr_2 = currVal_2;
    }
    this._compView_0.detectChanges();
    this._compView_8.detectChanges();
    this._compView_10.detectChanges();
    this._compView_17.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_24.destroyNestedViews();
    this._compView_0.destroyInternalState();
    this._compView_8.destroyInternalState();
    this._compView_10.destroyInternalState();
    this._compView_17.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J37FilhoSemProjecao, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J37FilhoSemProjecaoNgFactory = ComponentFactory<import1.J37FilhoSemProjecao>('j37-filho-sem-projecao', viewFactory_J37FilhoSemProjecaoHost0);
ComponentFactory<import1.J37FilhoSemProjecao> get J37FilhoSemProjecaoNgFactory {
  return _J37FilhoSemProjecaoNgFactory;
}

ComponentFactory<import1.J37FilhoSemProjecao> createJ37FilhoSemProjecaoFactory() {
  return ComponentFactory('j37-filho-sem-projecao', viewFactory_J37FilhoSemProjecaoHost0);
}

class _ViewJ37FilhoSemProjecao1 extends import17.EmbeddedView<import1.J37FilhoSemProjecao> {
  _ViewJ37FilhoSemProjecao1(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('em'));
    final _text_1 = import7.appendText(_el_0, 'w');
    this.initRootNode(_el_0);
  }
}

import17.EmbeddedView<void> viewFactory_J37FilhoSemProjecao1(import18.RenderView parentView, int parentIndex) {
  return _ViewJ37FilhoSemProjecao1(parentView, parentIndex);
}

final List<Object> styles$J37FilhoSemProjecaoHost = const [];

class _ViewJ37FilhoSemProjecaoHost0 extends import9.HostView<import1.J37FilhoSemProjecao> {
  @override
  void build() {
    this.componentView = ViewJ37FilhoSemProjecao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J37FilhoSemProjecao();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J37FilhoSemProjecao> viewFactory_J37FilhoSemProjecaoHost0() {
  return _ViewJ37FilhoSemProjecaoHost0();
}
