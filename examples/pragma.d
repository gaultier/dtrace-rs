#
#4
#5 "foo.d" 
#6 "foo.d" 1
#line 7 
#line 8 "foo.d" 
#line 9 "foo.d" 0
#line 10 "foo.d" 4

#pragma line 11 
#pragma line 12 "foo.d" 
#pragma line 13 "foo.d" 0
#pragma line 14 "foo.d" 4

#pragma D line 15 
#pragma D line 16 "foo.d" 
#pragma D line 17 "foo.d" 0
#pragma D line 18 "foo.d" 4

#ident 2
#pragma ident 3
#pragma D ident 3


#pragma bar
#pragma 

#pragma binding "255.2.3" foo
#pragma binding "255.2" foo

#pragma depends_on library darwin.d
#pragma depends_on module mach_kernel

BEGIN {
}
