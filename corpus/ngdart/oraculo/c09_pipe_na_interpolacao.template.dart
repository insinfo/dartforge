// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c09_pipe_na_interpolacao.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c09_pipe_na_interpolacao.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/common/pipes/uppercase_pipe.dart' as import3;
import 'dart:core';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'dart:html' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/runtime/proxies.dart' as import11;
import 'package:ngdart/src/runtime/interpolate.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import14;

final List<Object> styles$C09PipeNaInterpolacao = const [];

class ViewC09PipeNaInterpolacao0 extends import0.ComponentView<import1.C09PipeNaInterpolacao> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  late final import3.UpperCasePipe _pipe_uppercase_0;
  late final String? Function(String?) _pipe_uppercase_0_0;
  static import5.ComponentStyles? _componentStyles;
  ViewC09PipeNaInterpolacao0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('c09-pipe-na-interpolacao'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/c09_pipe_na_interpolacao.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import9.document;
    final _el_0 = import10.appendDiv(doc, parentRenderNode);
    _el_0.append(this._textBinding_1.element);
    this._pipe_uppercase_0 = import3.UpperCasePipe();
    this._pipe_uppercase_0_0 = import11.pureProxy1(this._pipe_uppercase_0.transform);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import12.interpolate0(this._pipe_uppercase_0_0(_ctx.nome))) /* REF:package:corpus_ngdart/src/c09_pipe_na_interpolacao.html:5:32 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$C09PipeNaInterpolacao, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C09PipeNaInterpolacaoNgFactory = ComponentFactory<import1.C09PipeNaInterpolacao>('c09-pipe-na-interpolacao', viewFactory_C09PipeNaInterpolacaoHost0);
ComponentFactory<import1.C09PipeNaInterpolacao> get C09PipeNaInterpolacaoNgFactory {
  return _C09PipeNaInterpolacaoNgFactory;
}

ComponentFactory<import1.C09PipeNaInterpolacao> createC09PipeNaInterpolacaoFactory() {
  return ComponentFactory('c09-pipe-na-interpolacao', viewFactory_C09PipeNaInterpolacaoHost0);
}

final List<Object> styles$C09PipeNaInterpolacaoHost = const [];

class _ViewC09PipeNaInterpolacaoHost0 extends import14.HostView<import1.C09PipeNaInterpolacao> {
  @override
  void build() {
    this.componentView = ViewC09PipeNaInterpolacao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C09PipeNaInterpolacao();
    this.initRootNode(_el_0);
  }
}

import14.HostView<import1.C09PipeNaInterpolacao> viewFactory_C09PipeNaInterpolacaoHost0() {
  return _ViewC09PipeNaInterpolacaoHost0();
}
