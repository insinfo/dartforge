// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'h02_campo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'h02_campo.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngforms/src/directives/control_value_accessor.dart' as import10;
import 'package:ngdart/src/meta/di_tokens.dart' as import11;
import 'package:ngforms/src/directives/control_value_accessor.dart' as import12;

final List<Object> styles$H02Campo = const [];

class ViewH02Campo0 extends import0.ComponentView<import1.H02Campo> {
  static import2.ComponentStyles? _componentStyles;
  ViewH02Campo0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('h02-campo'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/h02_campo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendSpan(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'campo');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$H02Campo, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _H02CampoNgFactory = ComponentFactory<import1.H02Campo>('h02-campo', viewFactory_H02CampoHost0);
ComponentFactory<import1.H02Campo> get H02CampoNgFactory {
  return _H02CampoNgFactory;
}

ComponentFactory<import1.H02Campo> createH02CampoFactory() {
  return ComponentFactory('h02-campo', viewFactory_H02CampoHost0);
}

final List<Object> styles$H02CampoHost = const [];

class _ViewH02CampoHost0 extends import9.HostView<import1.H02Campo> {
  late List<import10.ControlValueAccessor<dynamic>> _NgValueAccessor_0_6 = [this.component];
  @override
  void build() {
    this.componentView = ViewH02Campo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.H02Campo();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, const import11.MultiToken<import12.ControlValueAccessor<dynamic>>('NgValueAccessor')) && (0 == nodeIndex))) {
      return this._NgValueAccessor_0_6;
    }
    return notFoundResult;
  }
}

import9.HostView<import1.H02Campo> viewFactory_H02CampoHost0() {
  return _ViewH02CampoHost0();
}
