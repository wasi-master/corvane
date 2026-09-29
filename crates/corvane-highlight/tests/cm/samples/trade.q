/ q/kdb+ sample — gestion des trades
/ a single-line comment
\l utils.q
\d .trade
\ trailing text after backslash space
/
this whole block is a comment
until a line with only a backslash
\
x:1 2 3 / trailing comment
y:`sym`ibm`msft`goog`a.b/c:d
t:([] sym:`a`b`c; price:10.5 20.25 30f; size:100 200 300i)
f:{[a;b] a+b*2}
g:{x-y%z}
select avg price by sym from t where size>150
update vwap:size wavg price by sym from `t
exec sum size from t
d:2024.01.15
m:2024.01m
ts:2024.01.15D12:30:45.123456789
tp:2024.01.15T09:30:00.000
tm:12:30
tv:12:30:45.5
span:0D01:02:03
n:0N 0w 0W -0w
h:0x1f2e3d
b:0101b
c:42j 7h 3i 1c 2n
e:1.5e10 2e 3.25f -.5 .75 1e-3
bad:12abc 3.x
s:"a \"quoted\" string"
u:"unterminated
string spans lines"
if[x>2;show "big"]
do[3;-1 "hello"]
while[x<10;x+:1]
r:raze til 10
k:key `:data/trades
{x@&x>0} each (1 -2 3;-4 5)
.z.ts:{0N!.z.p}
@[`t;`price;*;1.1]
'`error
lj ij uj aj wj wj1 xasc xdesc
a:b,c,d;e#f;g$h;i?j;k!l;m=n;o~p;q<r;s>t
é:1 ü
	tab:`tabbed
/
multi-line block comment
opened by a lone slash /
\
z:last 1 2 3
\ts select from t
\\
