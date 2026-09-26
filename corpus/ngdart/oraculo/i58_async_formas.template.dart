// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i58_async_formas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i58_async_formas.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'a02_texto_estatico.template.dart' as import3;
import 'a02_texto_estatico.dart' as import4;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/common/pipes/uppercase_pipe.dart' as import7;
import 'dart:core';
import 'package:ngdart/src/common/pipes/async_pipe.dart' as import9;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import10;
import 'package:ngdart/src/core/linker/views/view.dart' as import11;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import12;
import 'package:ngdart/src/utilities.dart' as import13;
import 'dart:html' as import14;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import15;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import17;
import 'package:ngdart/src/runtime/proxies.dart' as import18;
import 'package:ngdart/src/runtime/interpolate.dart' as import19;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import21;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import22;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import23;

final List<Object> styles$I58AsyncFormas = const [];

class ViewI58AsyncFormas0 extends import0.ComponentView<import1.I58AsyncFormas> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  final import2.TextBinding _textBinding_7 = import2.TextBinding();
  late final import3.ViewA02TextoEstatico0 _compView_4;
  late final import4.A02TextoEstatico _A02TextoEstatico_4_5;
  late final ViewContainer _appEl_5;
  late final NgIf _NgIf_5_9;
  late final import7.UpperCasePipe _pipe_uppercase_0;
  late final String? Function(String?) _pipe_uppercase_0_0;
  late final import9.AsyncPipe _pipe_async_1;
  late final import9.AsyncPipe _pipe_async_2;
  static import10.ComponentStyles? _componentStyles;
  ViewI58AsyncFormas0(import11.View parentView, int parentIndex) : super(parentView, parentIndex, import12.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import13.unsafeCast(import14.document.createElement('i58-async-formas'));
  }
  static String? get _debugComponentUrl {
    return (import13.isDevMode ? 'asset:corpus_ngdart/lib/src/i58_async_formas.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import14.document;
    final _el_0 = import15.appendElement<import14.HtmlElement>(doc, parentRenderNode, 'p');
    _el_0.append(this._textBinding_1.element);
    final _text_2 = import15.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_3.element);
    this._compView_4 = import3.ViewA02TextoEstatico0(this, 4);
    final _el_4 = this._compView_4.rootElement;
    parentRenderNode.append(_el_4);
    this._A02TextoEstatico_4_5 = import4.A02TextoEstatico();
    this._compView_4.create(this._A02TextoEstatico_4_5);
    final _anchor_5 = import15.appendAnchor(parentRenderNode);
    this._appEl_5 = ViewContainer(5, null, this, _anchor_5);
    var _TemplateRef_5_8 = TemplateRef(this._appEl_5, viewFactory_I58AsyncFormas1);
    this._NgIf_5_9 = NgIf(this._appEl_5, _TemplateRef_5_8);
    if (import17.isDevToolsEnabled) {
      import17.Inspector.instance.registerDirective(_anchor_5, this._NgIf_5_9);
    }
    final _el_6 = import15.appendElement<import14.HtmlElement>(doc, parentRenderNode, 'b');
    _el_6.append(this._textBinding_7.element);
    this._pipe_uppercase_0 = import7.UpperCasePipe();
    this._pipe_uppercase_0_0 = import18.pureProxy1(this._pipe_uppercase_0.transform);
    this._pipe_async_1 = import9.AsyncPipe(this);
    this._pipe_async_2 = import9.AsyncPipe(this);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import17.isDevToolsEnabled) {
      import17.Inspector.instance.recordInput(this._NgIf_5_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_5_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/i58_async_formas.html:105:120 */;
    this._appEl_5.detectChangesInNestedViews();
    this._textBinding_1.updateText(import19.interpolate0(this._pipe_uppercase_0_0(_ctx.nome))) /* REF:package:corpus_ngdart/src/i58_async_formas.html:3:30 */;
    this._textBinding_3.updateText(import19.interpolate0(this._pipe_async_1.transform(_ctx.fluxo))) /* REF:package:corpus_ngdart/src/i58_async_formas.html:31:55 */;
    this._textBinding_7.updateText(import19.interpolate0(this._pipe_async_2.transform(_ctx.futuro))) /* REF:package:corpus_ngdart/src/i58_async_formas.html:180:205 */;
    this._compView_4.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_5.destroyNestedViews();
    this._compView_4.destroyInternalState();
    this._pipe_async_1.ngOnDestroy();
    this._pipe_async_2.ngOnDestroy();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import10.ComponentStyles.unscoped(styles$I58AsyncFormas, _debugComponentUrl));
      if (import13.isDevMode) {
        import10.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I58AsyncFormasNgFactory = ComponentFactory<import1.I58AsyncFormas>('i58-async-formas', viewFactory_I58AsyncFormasHost0);
ComponentFactory<import1.I58AsyncFormas> get I58AsyncFormasNgFactory {
  return _I58AsyncFormasNgFactory;
}

ComponentFactory<import1.I58AsyncFormas> createI58AsyncFormasFactory() {
  return ComponentFactory('i58-async-formas', viewFactory_I58AsyncFormasHost0);
}

class _ViewI58AsyncFormas1 extends import21.EmbeddedView<import1.I58AsyncFormas> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  late final import9.AsyncPipe _pipe_async_0;
  late final import9.AsyncPipe _pipe_async_1;
  _ViewI58AsyncFormas1(import22.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import14.document;
    final _el_0 = import13.unsafeCast(doc.createElement('div'));
    _el_0.append(this._textBinding_1.element);
    final _text_2 = import15.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_3.element);
    this._pipe_async_0 = import9.AsyncPipe(this);
    this._pipe_async_1 = import9.AsyncPipe(this);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import19.interpolate0(this._pipe_async_0.transform(_ctx.fluxo))) /* REF:package:corpus_ngdart/src/i58_async_formas.html:121:145 */;
    this._textBinding_3.updateText(import19.interpolate0(this._pipe_async_1.transform(_ctx.futuro))) /* REF:package:corpus_ngdart/src/i58_async_formas.html:146:171 */;
  }

  @override
  void destroyInternal() {
    this._pipe_async_0.ngOnDestroy();
    this._pipe_async_1.ngOnDestroy();
  }
}

import21.EmbeddedView<void> viewFactory_I58AsyncFormas1(import22.RenderView parentView, int parentIndex) {
  return _ViewI58AsyncFormas1(parentView, parentIndex);
}

final List<Object> styles$I58AsyncFormasHost = const [];

class _ViewI58AsyncFormasHost0 extends import23.HostView<import1.I58AsyncFormas> {
  @override
  void build() {
    this.componentView = ViewI58AsyncFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I58AsyncFormas();
    this.initRootNode(_el_0);
  }
}

import23.HostView<import1.I58AsyncFormas> viewFactory_I58AsyncFormasHost0() {
  return _ViewI58AsyncFormasHost0();
}
