// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i29_async_pipe.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i29_async_pipe.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/common/pipes/async_pipe.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/runtime/interpolate.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$I29AsyncPipe = const [];

class ViewI29AsyncPipe0 extends import0.ComponentView<import1.I29AsyncPipe> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  late final import3.AsyncPipe _pipe_async_0;
  static import4.ComponentStyles? _componentStyles;
  ViewI29AsyncPipe0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i29-async-pipe'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i29_async_pipe.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendElement<import8.HtmlElement>(doc, parentRenderNode, 'p');
    _el_0.append(this._textBinding_1.element);
    this._pipe_async_0 = import3.AsyncPipe(this);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import10.interpolate0(this._pipe_async_0.transform(_ctx.fluxo))) /* REF:package:corpus_ngdart/src/i29_async_pipe.html:3:27 */;
  }

  @override
  void destroyInternal() {
    this._pipe_async_0.ngOnDestroy();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I29AsyncPipe, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I29AsyncPipeNgFactory = ComponentFactory<import1.I29AsyncPipe>('i29-async-pipe', viewFactory_I29AsyncPipeHost0);
ComponentFactory<import1.I29AsyncPipe> get I29AsyncPipeNgFactory {
  return _I29AsyncPipeNgFactory;
}

ComponentFactory<import1.I29AsyncPipe> createI29AsyncPipeFactory() {
  return ComponentFactory('i29-async-pipe', viewFactory_I29AsyncPipeHost0);
}

final List<Object> styles$I29AsyncPipeHost = const [];

class _ViewI29AsyncPipeHost0 extends import12.HostView<import1.I29AsyncPipe> {
  @override
  void build() {
    this.componentView = ViewI29AsyncPipe0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I29AsyncPipe();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.I29AsyncPipe> viewFactory_I29AsyncPipeHost0() {
  return _ViewI29AsyncPipeHost0();
}
