int first() { final int a; (a,a) = (1,2); return a; }
int second() { final a=1; (a,a) = (2,3); return a; }
int rhs() { final int a; (a,) = (a=1,); return a; }
int loop() { final int a; for(var i=0;i<2;i++) { (a,)=(i,); } return a; }
