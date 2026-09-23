// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a26_constantes_em_embutida.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a26_constantes_em_embutida.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
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

final List<Object> styles$A26ConstantesEmEmbutida = const [];

class ViewA26ConstantesEmEmbutida0 extends import0.ComponentView<import1.A26ConstantesEmEmbutida> {
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  late final ViewContainer _appEl_2;
  late final import4.NgFor _NgFor_2_9;
  static import5.ComponentStyles? _componentStyles;
  ViewA26ConstantesEmEmbutida0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('a26-constantes-em-embutida'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/a26_constantes_em_embutida.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import10.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_A26ConstantesEmEmbutida1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
    final doc = import9.document;
    final _el_1 = import10.appendElement<import9.UListElement>(doc, parentRenderNode, 'ul');
    final _anchor_2 = import10.appendAnchor(_el_1);
    this._appEl_2 = ViewContainer(2, 1, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_A26ConstantesEmEmbutida2);
    this._NgFor_2_9 = import4.NgFor(this._appEl_2, _TemplateRef_2_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_2, this._NgFor_2_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.fixo);
      }
      this._NgIf_0_9.ngIf = _ctx.fixo /* REF:package:corpus_ngdart/src/a26_constantes_em_embutida.html:5:17 */;
      if ((_ctx.itens != null)) {
        if (import12.isDevToolsEnabled) {
          import12.Inspector.instance.recordInput(this._NgFor_2_9, 'ngForOf', _ctx.itens);
        }
        this._NgFor_2_9.ngForOf = _ctx.itens /* REF:package:corpus_ngdart/src/a26_constantes_em_embutida.html:54:77 */;
      }
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgFor_2_9.ngDoCheck();
    }
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_2.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this._appEl_2.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$A26ConstantesEmEmbutida, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A26ConstantesEmEmbutidaNgFactory = ComponentFactory<import1.A26ConstantesEmEmbutida>('a26-constantes-em-embutida', viewFactory_A26ConstantesEmEmbutidaHost0);
ComponentFactory<import1.A26ConstantesEmEmbutida> get A26ConstantesEmEmbutidaNgFactory {
  return _A26ConstantesEmEmbutidaNgFactory;
}

ComponentFactory<import1.A26ConstantesEmEmbutida> createA26ConstantesEmEmbutidaFactory() {
  return ComponentFactory('a26-constantes-em-embutida', viewFactory_A26ConstantesEmEmbutidaHost0);
}

class _ViewA26ConstantesEmEmbutida1 extends import15.EmbeddedView<import1.A26ConstantesEmEmbutida> {
  late final import9.HtmlElement _el_1;
  _ViewA26ConstantesEmEmbutida1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('div'));
    this._el_1 = import10.appendElement<import9.HtmlElement>(doc, _el_0, 'p');
    final _text_2 = import10.appendText(this._el_1, 'a');
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      import10.setProperty(this._el_1, 'title', 'x') /* REF:package:corpus_ngdart/src/a26_constantes_em_embutida.html:21:34 */;
    }
  }
}

import15.EmbeddedView<void> viewFactory_A26ConstantesEmEmbutida1(import16.RenderView parentView, int parentIndex) {
  return _ViewA26ConstantesEmEmbutida1(parentView, parentIndex);
}

class _ViewA26ConstantesEmEmbutida2 extends import15.EmbeddedView<import1.A26ConstantesEmEmbutida> {
  final import17.TextBinding _textBinding_1 = import17.TextBinding();
  _ViewA26ConstantesEmEmbutida2(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('li'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_x = import8.unsafeCast<String>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import19.interpolateString0(local_x)) /* REF:package:corpus_ngdart/src/a26_constantes_em_embutida.html:78:83 */;
  }
}

import15.EmbeddedView<void> viewFactory_A26ConstantesEmEmbutida2(import16.RenderView parentView, int parentIndex) {
  return _ViewA26ConstantesEmEmbutida2(parentView, parentIndex);
}

final List<Object> styles$A26ConstantesEmEmbutidaHost = const [];

class _ViewA26ConstantesEmEmbutidaHost0 extends import20.HostView<import1.A26ConstantesEmEmbutida> {
  @override
  void build() {
    this.componentView = ViewA26ConstantesEmEmbutida0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A26ConstantesEmEmbutida();
    this.initRootNode(_el_0);
  }
}

import20.HostView<import1.A26ConstantesEmEmbutida> viewFactory_A26ConstantesEmEmbutidaHost0() {
  return _ViewA26ConstantesEmEmbutidaHost0();
}
