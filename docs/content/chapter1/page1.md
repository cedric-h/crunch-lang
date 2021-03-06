+++
title = "Page 1"
weight = 1
+++

```ebnf
Program ::= ( FunctionDeclaration | TypeDeclaration | Import )*

FunctionDeclaration ::= Visibility? 'fn' Ident ( '<' Ident ','? '>' )? '(' FunctionArguments* ')' ( '->' Ident )? '\n' Body End
FunctionArguments ::= ( Ident ( ':' Ident )? ) | ( FunctionArguments ',' )

TypeDeclaration ::= Visibility? 'type' Ident ( '<' Ident ',' '>' )? '\n' TypeArguments* Function* End
TypeArguments ::= ( ( Ident ':' Ident ) | TypeArguments ',' ) '\n'

Import ::= 'import' ImportDestination? String ( 'exposing' ( '*' | ( Ident ( 'as' Ident )? ',' ? )+ ) )? '\n'
ImportDestination ::= 'lib' | 'pkg'

VarDeclaration ::= 'let' Ident ( ':' Ident )? '=' Expr '\n'
Assignment ::= Ident Assigner Expr '\n'
FunctionCall ::= Ident ( '.' Ident )? '(' FunctionCallArgs* ')' '\n'
FunctionCallArgs ::= Expr | ( FunctionCallArgs ',' Expr )

If ::= 'if' Comparison '\n' Body ElseIf* Else? End
ElseIf ::= 'else if' Comparison '\n' Body
Else ::= 'else' '\n' Body
InlineIf ::= 'if' Expr 'then' Expr 'else' Expr /* Usage of an inline if statement is still tbd */

While ::= 'while' Expr '\n' Body ( 'then' '\n' Body )? End
Loop ::= 'loop' Body 'end'
For ::= 'for' Ident 'in' Expr '\n' Body ( 'then' '\n' Body )?  End

BinaryOperation ::= Expr BinaryOperand Expr
Comparison ::= Expr Comparator Expr

BinaryOperand ::= ( '+' | '-' | '*' | '/' | '^' | '|' | '&' ) ( '?' | '!' )?
Comparator ::= '==' | '<=' | '>=' | '<' | '>'
Assigner ::= '=' | ( BinaryOperand '=' )

Ident ::= [a-zA-Z]+
Literal ::= String | Integer | Boolean
String ::= '"' [^"]* '"' | "'" [^']* "'"
Integer ::= [0-9]+
Boolean ::= 'true' | 'false'
Range ::= Expr '..' Expr

Return ::= 'return' Expr '\n'
Continue ::= 'continue' '\n'
Break ::= 'break' '\n'

Visibility ::= 'exposed' | 'private'
End ::= 'end'

Expr ::= Literal | Range | Comparison | BinaryOperation | Ident | ( '(' Expr ')' )

Statement ::= If | While | Loop | For | FunctionCall | Assignment | VarDeclaration | Return | Continue | Break | ( Expr '\n' )

Body ::= Statement+ | 'empty'
```
