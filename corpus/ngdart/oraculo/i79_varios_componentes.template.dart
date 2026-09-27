// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i79_varios_componentes.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i79_varios_componentes.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/check_binding.dart' as import7;
import 'package:ngdart/src/devtools.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;
import 'package:ngdart/src/runtime/text_binding.dart' as import11;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import12;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;

final List<Object> styles$I79VariosComponentes = const [];

class ViewI79VariosComponentes0 extends import0.ComponentView<import1.I79VariosComponentes> {
  late final ViewI79Folha0 _compView_0;
  late final import1.I79Folha _I79Folha_0_5;
  late final ViewI79Outra0 _compView_1;
  late final import1.I79Outra _I79Outra_1_5;
  Object? _expr_0;
  static import2.ComponentStyles? _componentStyles;
  ViewI79VariosComponentes0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i79-varios-componentes'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i79_varios_componentes.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewI79Folha0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._I79Folha_0_5 = import1.I79Folha();
    this._compView_0.create(this._I79Folha_0_5);
    this._compView_1 = ViewI79Outra0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    this._I79Outra_1_5 = import1.I79Outra();
    this._compView_1.create(this._I79Outra_1_5);
    _el_0.addEventListener('click', this.eventHandler1(this._handleEvent_0));
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.n;
    if (import7.checkBinding(this._expr_0, currVal_0, 'n', 'asset:corpus_ngdart/lib/src/i79_varios_componentes.dart')) {
      if (import8.isDevToolsEnabled) {
        import8.Inspector.instance.recordInput(this._I79Folha_0_5, 'valor', currVal_0);
      }
      this._I79Folha_0_5.valor = currVal_0 /* REF:asset:corpus_ngdart/lib/src/i79_varios_componentes.dart:369:380 */;
      this._expr_0 = currVal_0;
    }
    this._compView_0.detectChanges();
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_1.destroyInternalState();
  }

  void _handleEvent_0($event) {
    final _ctx = this.ctx;
    _ctx.n = (_ctx.n + 1);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I79VariosComponentes, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I79VariosComponentesNgFactory = ComponentFactory<import1.I79VariosComponentes>('i79-varios-componentes', viewFactory_I79VariosComponentesHost0);
ComponentFactory<import1.I79VariosComponentes> get I79VariosComponentesNgFactory {
  return _I79VariosComponentesNgFactory;
}

ComponentFactory<import1.I79VariosComponentes> createI79VariosComponentesFactory() {
  return ComponentFactory('i79-varios-componentes', viewFactory_I79VariosComponentesHost0);
}

final List<Object> styles$I79VariosComponentesHost = const [];

class _ViewI79VariosComponentesHost0 extends import10.HostView<import1.I79VariosComponentes> {
  @override
  void build() {
    this.componentView = ViewI79VariosComponentes0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I79VariosComponentes();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I79VariosComponentes> viewFactory_I79VariosComponentesHost0() {
  return _ViewI79VariosComponentesHost0();
}

final List<Object> styles$I79Folha = const [];

class ViewI79Folha0 extends import0.ComponentView<import1.I79Folha> {
  final import11.TextBinding _textBinding_1 = import11.TextBinding();
  final import11.TextBinding _textBinding_3 = import11.TextBinding();
  static import2.ComponentStyles? _componentStyles;
  ViewI79Folha0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i79-folha'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i79_varios_componentes.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import12.appendSpan(doc, parentRenderNode);
    _el_0.append(this._textBinding_1.element);
    final _text_2 = import12.appendText(_el_0, ' é ');
    _el_0.append(this._textBinding_3.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateTextWithPrimitive(_ctx.valor) /* REF:asset:corpus_ngdart/lib/src/i79_varios_componentes.dart:573:582 */;
    this._textBinding_3.updateTextWithPrimitive(_ctx.valor) /* REF:asset:corpus_ngdart/lib/src/i79_varios_componentes.dart:585:594 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I79Folha, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I79FolhaNgFactory = ComponentFactory<import1.I79Folha>('i79-folha', viewFactory_I79FolhaHost0);
ComponentFactory<import1.I79Folha> get I79FolhaNgFactory {
  return _I79FolhaNgFactory;
}

ComponentFactory<import1.I79Folha> createI79FolhaFactory() {
  return ComponentFactory('i79-folha', viewFactory_I79FolhaHost0);
}

final List<Object> styles$I79FolhaHost = const [];

class _ViewI79FolhaHost0 extends import10.HostView<import1.I79Folha> {
  @override
  void build() {
    this.componentView = ViewI79Folha0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I79Folha();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I79Folha> viewFactory_I79FolhaHost0() {
  return _ViewI79FolhaHost0();
}

final List<Object> styles$I79Outra = const [];

class ViewI79Outra0 extends import0.ComponentView<import1.I79Outra> {
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  static import2.ComponentStyles? _componentStyles;
  ViewI79Outra0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i79-outra'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i79_varios_componentes.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import12.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_I79Outra1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.ver);
    }
    this._NgIf_0_9.ngIf = _ctx.ver /* REF:package:corpus_ngdart/src/i79_varios_componentes.html:3:14 */;
    this._appEl_0.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I79Outra, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I79OutraNgFactory = ComponentFactory<import1.I79Outra>('i79-outra', viewFactory_I79OutraHost0);
ComponentFactory<import1.I79Outra> get I79OutraNgFactory {
  return _I79OutraNgFactory;
}

ComponentFactory<import1.I79Outra> createI79OutraFactory() {
  return ComponentFactory('i79-outra', viewFactory_I79OutraHost0);
}

class _ViewI79Outra1 extends import16.EmbeddedView<import1.I79Outra> {
  _ViewI79Outra1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('p'));
    final _text_1 = import12.appendText(_el_0, 'oi');
    this.initRootNode(_el_0);
  }
}

import16.EmbeddedView<void> viewFactory_I79Outra1(import17.RenderView parentView, int parentIndex) {
  return _ViewI79Outra1(parentView, parentIndex);
}

final List<Object> styles$I79OutraHost = const [];

class _ViewI79OutraHost0 extends import10.HostView<import1.I79Outra> {
  @override
  void build() {
    this.componentView = ViewI79Outra0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I79Outra();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I79Outra> viewFactory_I79OutraHost0() {
  return _ViewI79OutraHost0();
}
