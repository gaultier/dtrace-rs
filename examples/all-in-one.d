struct  Point  {  int  x  ;  int  y  ;  }  ;

struct  Node  ;

union  Value  {  int  i  ;  char  c  ;  }  ;

enum  Color  {  RED  =  0  ,  GREEN  =  1  ,  BLUE  =  2  }  ;

inline  int  MAX_SIZE  =  1024  ;

int  *ptr  ;
const  int  * const  cptr  ;
int  **dptr  ;
int  foo  (  int  a  ,  int  b  )  ;

BEGIN  ,  END  {  }

syscall::open:entry
/  pid  ==  42  /
{
  x  =  a  +  b  ;
  x  =  a  -  b  ;
  x  =  a  *  b  ;
  x  =  a  /  b  ;
  x  =  a  %  b  ;
  x  =  a  &  b  ;
  x  =  a  |  b  ;
  x  =  a  ^  b  ;
  x  =  a  <<  b  ;
  x  =  a  >>  b  ;
  x  =  a  ==  b  ;
  x  =  a  !=  b  ;
  x  =  a  <  b  ;
  x  =  a  >  b  ;
  x  =  a  <=  b  ;
  x  =  a  >=  b  ;
  x  =  a  &&  b  ;
  x  =  a  ||  b  ;
  x  =  1  ;
  x  +=  1  ;
  x  -=  1  ;
  x  =  -  y  ;
  x  =  !  y  ;
  x  =  ~  y  ;
  x  =  *  y  ;
  x  =  &  y  ;
  ++  x  ;
  --  x  ;
  x  ++  ;
  x  --  ;
  print  (  )  ;
  print  (  a  )  ;
  print  (  a  ,  b  ,  c  )  ;
  x  =  a  [  i  ]  ;
  x  =  a  [  i  ]  [  j  ]  ;
  x  =  a  .  b  ;
  x  =  a  ->  b  ;
  x  =  a  .  b  .  c  ;
  x  =  a  ?  b  :  c  ;
  x  =  a  ,  b  ;
  x  =  sizeof  (  int  )  ;
  x  =  sizeof  (  const  int  )  ;
  x  =  sizeof  (  int  *  )  ;
  x  =  sizeof  (  int  * const  )  ;
  x  =  sizeof   y  ;
  x  =  stringof  (  y  )  ;
  x  =  stringof   y  ;
  x  =  (  mytype  )  y  ;
  n  =  offsetof  (  int  ,  field  )  ;
  x  =  xlate  <  int  >  (  ptr  )  ;
  @n  ++  ;
  x  =  'a'  ;
  x  =  "hello"  ;
  if  (  x  ==  1  )  {  y  =  2  ;  }
  if  (  x  ==  1  )  {  y  =  2  ;  }  else  {  y  =  3  ;  }
  if  (  x  ==  1  )  {  y  =  2  ;  }  else  if  (  x  ==  2  )  {  y  =  3  ;  }  else  {  y  =  4  ;  }
}
