// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b10_after_view_checked.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b10_after_view_checked.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;

final List<Object> styles$B10AfterViewChecked = const [];

class ViewB10AfterViewChecked0 extends import0.ComponentView<import1.B10AfterViewChecked> {
  static import2.ComponentStyles? _componentStyles;
  ViewB10AfterViewChecked0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b10-after-view-checked'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b10_after_view_checked.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'oi');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B10AfterViewChecked, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B10AfterViewCheckedNgFactory = ComponentFactory<import1.B10AfterViewChecked>('b10-after-view-checked', viewFactory_B10AfterViewCheckedHost0);
ComponentFactory<import1.B10AfterViewChecked> get B10AfterViewCheckedNgFactory {
  return _B10AfterViewCheckedNgFactory;
}

ComponentFactory<import1.B10AfterViewChecked> createB10AfterViewCheckedFactory() {
  return ComponentFactory('b10-after-view-checked', viewFactory_B10AfterViewCheckedHost0);
}

final List<Object> styles$B10AfterViewCheckedHost = const [];

class _ViewB10AfterViewCheckedHost0 extends import9.HostView<import1.B10AfterViewChecked> {
  @override
  void build() {
    this.componentView = ViewB10AfterViewChecked0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B10AfterViewChecked();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    this.componentView.detectChanges();
    if ((!import10.debugThrowIfChanged)) {
      this.component.ngAfterViewChecked();
    }
  }
}

import9.HostView<import1.B10AfterViewChecked> viewFactory_B10AfterViewCheckedHost0() {
  return _ViewB10AfterViewCheckedHost0();
}
