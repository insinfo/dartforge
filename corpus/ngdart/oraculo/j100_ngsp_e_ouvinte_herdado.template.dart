// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j100_ngsp_e_ouvinte_herdado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j100_ngsp_e_ouvinte_herdado.dart' as import1;
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
import 'package:ngdart/src/runtime/text_binding.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;
import 'package:ngdart/src/runtime/interpolate.dart' as import16;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import17;

final List<Object> styles$J100Ngsp = const [];

class ViewJ100Ngsp0 extends import0.ComponentView<import1.J100Ngsp> {
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  static import4.ComponentStyles? _componentStyles;
  ViewJ100Ngsp0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j100-ngsp'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j100_ngsp_e_ouvinte_herdado.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J100Ngsp1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
    final doc = import8.document;
    final _el_1 = import9.appendElement<import8.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_2 = import9.appendText(_el_1, ' fim ');
    parentRenderNode.addEventListener('keydown', this.eventHandler1(_ctx.tecla));
    parentRenderNode.addEventListener('blur', this.eventHandler0(_ctx.saiu));
    parentRenderNode.addEventListener('focus', this.eventHandler1(_ctx.entrou));
    parentRenderNode.addEventListener('mousedown', this.eventHandler0(_ctx.apertou));
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', (_ctx.descricao != null));
    }
    this._NgIf_0_9.ngIf = (_ctx.descricao != null) /* REF:asset:corpus_ngdart/lib/src/j100_ngsp_e_ouvinte_herdado.dart:572:597 */;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J100Ngsp, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J100NgspNgFactory = ComponentFactory<import1.J100Ngsp>('j100-ngsp', viewFactory_J100NgspHost0);
ComponentFactory<import1.J100Ngsp> get J100NgspNgFactory {
  return _J100NgspNgFactory;
}

ComponentFactory<import1.J100Ngsp> createJ100NgspFactory() {
  return ComponentFactory('j100-ngsp', viewFactory_J100NgspHost0);
}

class _ViewJ100Ngsp1 extends import13.EmbeddedView<import1.J100Ngsp> {
  final import14.TextBinding _textBinding_3 = import14.TextBinding();
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  _ViewJ100Ngsp1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('span'));
    this.updateChildClass(_el_0, 'd');
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J100Ngsp2);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
    final _text_2 = import9.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_3.element);
    final _text_4 = import9.appendText(_el_0, '  ');
    this.project(_el_0, 0);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.mostra);
    }
    this._NgIf_1_9.ngIf = _ctx.mostra /* REF:asset:corpus_ngdart/lib/src/j100_ngsp_e_ouvinte_herdado.dart:616:630 */;
    this._appEl_1.detectChangesInNestedViews();
    this._textBinding_3.updateText(import16.interpolateString0(_ctx.descricao)) /* REF:asset:corpus_ngdart/lib/src/j100_ngsp_e_ouvinte_herdado.dart:659:672 */;
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import13.EmbeddedView<void> viewFactory_J100Ngsp1(import15.RenderView parentView, int parentIndex) {
  return _ViewJ100Ngsp1(parentView, parentIndex);
}

class _ViewJ100Ngsp2 extends import13.EmbeddedView<import1.J100Ngsp> {
  _ViewJ100Ngsp2(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    this.updateChildClass(_el_0, 'g');
    this.initRootNode(_el_0);
  }
}

import13.EmbeddedView<void> viewFactory_J100Ngsp2(import15.RenderView parentView, int parentIndex) {
  return _ViewJ100Ngsp2(parentView, parentIndex);
}

final List<Object> styles$J100NgspHost = const [];

class _ViewJ100NgspHost0 extends import17.HostView<import1.J100Ngsp> {
  @override
  void build() {
    this.componentView = ViewJ100Ngsp0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J100Ngsp();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.J100Ngsp> viewFactory_J100NgspHost0() {
  return _ViewJ100NgspHost0();
}
