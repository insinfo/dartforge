main() {
  int i = 0;
  switch(i)
    L:
  {
    case 111:
      while (false) {
        break L;
      }
      i++;
  }
}
