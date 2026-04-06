#pragma binding "255.4095.4095" foo 
#pragma D attributes Stable/Stable/Common foo
#pragma D binding "" foo
#pragma D binding "1" foo
#pragma D binding "1.2.3.4" foo
#pragma D depends_on library bar
#pragma option foo
    #pragma option foo=bar
#pragma option foo bar

inline int* a /* bar */ = 123;

BEGIN /1/{
  // foo
  // foo // bar

 132 /* some comment */ + 456
}
