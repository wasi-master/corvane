"Counter — a tiny Smalltalk class
 with a comment that spans
 several lines"
Object subclass: #Counter
	instanceVariableNames: 'count step'
	classVariableNames: ''
	package: 'Demo-Counters'.

Counter >> initialize [
	super initialize.
	count := 0.
	step := 1
]

Counter >> increment [
	"Answer the new value"
	| old new |
	old := count.
	new := old + step.
	count := new.
	^ self
]

Counter >> incrementBy: anInteger [
	anInteger < 0 ifTrue: [ ^ self error: 'negative step' ].
	count := count + anInteger.
	^ count
]

Counter class >> demo [
	| c results |
	c := self new.
	c increment; increment; incrementBy: 16r1F.
	results := OrderedCollection new.
	#(1 2 3 $a #sym 'str' 2.5e10 -7) do: [ :each | results add: each printString ].
	Transcript show: c count printString; cr.
	results isEmpty ifFalse: [ Transcript show: 'non-empty' ].
	^ { c. results. #+. #at:put:. #'with space'. $<. $ . thisContext. nil. true. false }
]

Counter >> printOn: aStream [
	aStream nextPutAll: 'Counter('; print: count; nextPut: $).
	aStream << 'ünïcödé ✓ 😀' ; yourself
]

Counter >> itself [ ^ myself ]
"unterminated comment
continues on this line
and ends" here isNil.
'a string
over two lines' size.
#'symbol
split' , #foo:bar: , # , #[1 2 3].
[:x :y | x @ y ] value: 3 value: 4.
x := 3 > 2 | (4 >= 5) & ( 1 ~= 2 ) ==> 7 \\ 2 // 3.
Smalltalk at: #Foo put: 42 ; at: #Bar put: 7r16 .
¿ ¡ weird → chars
'unterminated at eof
