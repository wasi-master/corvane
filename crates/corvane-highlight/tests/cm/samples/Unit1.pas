{ Unit1 — main form of the Corvane demo
  multi-line brace comment, ünïcödé }
unit Unit1;

{$mode objfpc}{$H+}

interface

uses
  Classes, SysUtils, Forms, Controls, Graphics, Dialogs, StdCtrls;

type
  TPoint3D = record
    X, Y, Z: Double;
  end;

  TShape = class abstract(TObject)
  private
    FName: string;
    FArea: Extended;
  protected
    function GetArea: Extended; virtual; abstract;
  public
    constructor Create(const AName: string);
    destructor Destroy; override;
    property Name: string read FName write FName;
    property Area: Extended read GetArea;
  end;

  TCircle = class(TShape)
  strict private
    FRadius: Double;
  public
    function GetArea: Extended; override;
  end;

  TForm1 = class(TForm)
    Button1: TButton;
    procedure Button1Click(Sender: TObject);
  end;

  TColorSet = set of (clRed, clGreen, clBlue);
  TMatrix = array[0..3, 0..3] of Single;
  PInteger = ^Integer;

var
  Form1: TForm1;
  Counter: Integer = 0;

const
  MaxItems = 100;
  Pi2 = 6.28318;
  HexVal = $FF;
  BinVal = %1010;
  OctVal = &17;
  Greeting = 'Hello, ''world''!';
  Escaped = 'back\slash\';
  DoubleQ = "double quoted";
  CharCode = #13#10;
  Mixed = 'Line1'#13#10'Line2';
  Unicode = 'naïve café 日本語 🎉';

implementation

{$R *.lfm}

(* Old-style comment
   spanning lines *)

constructor TShape.Create(const AName: string);
begin
  inherited Create;
  FName := AName;
end;

destructor TShape.Destroy;
begin
  inherited Destroy;
end;

function TCircle.GetArea: Extended;
begin
  Result := Pi * FRadius * FRadius;
end;

procedure TForm1.Button1Click(Sender: TObject);
var
  i, j: Integer;
  s: string;
  p: PInteger;
  shape: TShape;
begin
  // single line comment
  for i := 1 to MaxItems do
  begin
    if (i mod 2 = 0) and not (i div 3 > 10) or (i xor 1 <> 0) then
      Inc(Counter)
    else if i shl 2 >= 64 then
      Dec(Counter, 2);
  end;
  for j := 10 downto 0 do Continue;
  while Counter > 0 do Counter -= 1;
  repeat
    Counter += 1;
  until Counter >= 5;
  case Counter of
    0: s := 'zero';
    1..4: s := 'few';
  else
    s := 'many';
  end;
  with Form1 do Caption := s;
  shape := TCircle.Create('circle');
  try
    try
      ShowMessage(Format('%s: %.2f', [shape.Name, shape.Area]));
    except
      on E: Exception do raise;
    end;
  finally
    shape.Free;
  end;
  p := @Counter;
  p^ := 3.14e2;
  if shape is TCircle then (shape as TCircle).Free;
  if p = nil then Exit;
  s := null;
  Begin End IF Then; // upper case is not a keyword
  x := 1.5.6e;
  a := b / c * d - e + f;
  x!?|&y;
end;

initialization
  Counter := 0;

finalization
  Counter := -1;

end.
{ unterminated brace comment
still inside
(* unterminated paren comment
'unterminated string
'ends with backslash \