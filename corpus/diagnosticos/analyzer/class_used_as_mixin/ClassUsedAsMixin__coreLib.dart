class Bar with Comparable<int> {
//             ^^^^^^^^^^^^^^^
// [diag.classUsedAsMixin] The class 'Comparable' can't be used as a mixin because it's neither a mixin class nor a mixin.
  int compareTo(int x) => 0;
}
