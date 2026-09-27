// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i98_view_child_read.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i98_view_child_read.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/core/linker/element_ref.dart';
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$I98ViewChildRead = const [];

class ViewI98ViewChildRead0 extends import0.ComponentView<import1.I98ViewChildRead> {
  static import2.ComponentStyles? _componentStyles;
  ViewI98ViewChildRead0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i98-view-child-read'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i98_view_child_read.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'x');
    _ctx.ref = ElementRef(_el_0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I98ViewChildRead, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I98ViewChildReadNgFactory = ComponentFactory<import1.I98ViewChildRead>('i98-view-child-read', viewFactory_I98ViewChildReadHost0);
ComponentFactory<import1.I98ViewChildRead> get I98ViewChildReadNgFactory {
  return _I98ViewChildReadNgFactory;
}

ComponentFactory<import1.I98ViewChildRead> createI98ViewChildReadFactory() {
  return ComponentFactory('i98-view-child-read', viewFactory_I98ViewChildReadHost0);
}

final List<Object> styles$I98ViewChildReadHost = const [];

class _ViewI98ViewChildReadHost0 extends import10.HostView<import1.I98ViewChildRead> {
  @override
  void build() {
    this.componentView = ViewI98ViewChildRead0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I98ViewChildRead();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I98ViewChildRead> viewFactory_I98ViewChildReadHost0() {
  return _ViewI98ViewChildReadHost0();
}
