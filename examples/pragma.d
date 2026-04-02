#
#4
#5 "foo.d" 
#5 "foo.d" 1
#line 5 
#line 5 "foo.d" 
#line 5 "foo.d" 0
#line 5 "foo.d" 4

#error 
#error foo 
#error foo bar baz

#pragma error 
#pragma error foo 
#pragma error foo bar baz


#pragma line 5 
#pragma line 5 "foo.d" 
#pragma line 5 "foo.d" 0
#pragma line 5 "foo.d" 4

#ident 2

BEGIN {
}
