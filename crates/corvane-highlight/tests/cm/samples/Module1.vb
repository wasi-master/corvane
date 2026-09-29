' Module1.vb — inventory console app
' Copyright © 2026 Ünïcode Corp.
Option Strict On
Option Explicit On
Option Infer Off

Imports System
Imports System.Collections.Generic
Imports System.Linq
Imports System.Threading.Tasks

#Const DEBUG_MODE = True
#Region "Constants"
#If DEBUG_MODE Then
#Else
#End If

Namespace Inventory

    Public Enum Category As Byte
        Hardware = 1
        Software = &H2
        Services = &O7
    End Enum

    Public Structure Money
        Public Amount As Decimal
        Public Currency As String
        Public Overrides Function ToString() As String
            Return Amount.ToString("F2") & " " & Currency
        End Function
    End Structure

    Public Interface IStockItem
        ReadOnly Property Sku As String
        Property Quantity As Integer
        Event Changed(sender As Object, e As EventArgs)
    End Interface

    <Serializable()>
    Public MustInherit Class ItemBase
        Implements IStockItem

        Private ReadOnly _sku As String
        Private _qty As Integer = 0
        Protected Friend Shared Count As Long = 0L

        Public Event Changed(sender As Object, e As EventArgs) Implements IStockItem.Changed

        Protected Sub New(ByVal sku As String)
            _sku = sku
            Count += 1
        End Sub

        Public ReadOnly Property Sku As String Implements IStockItem.Sku
            Get
                Return _sku
            End Get
        End Property

        Public Property Quantity As Integer Implements IStockItem.Quantity
            Get
                Return _qty
            End Get
            Set(value As Integer)
                If value < 0 Then Throw New ArgumentOutOfRangeException(NameOf(value))
                _qty = value
                RaiseEvent Changed(Me, EventArgs.Empty)
            End Set
        End Property

        Public MustOverride Function Price() As Money
    End Class

    Public NotInheritable Class Widget
        Inherits ItemBase

        Public Sub New(sku As String)
            MyBase.New(sku)
        End Sub

        Public Overrides Function Price() As Money
            Return New Money With {.Amount = 9.99D, .Currency = "EUR"}
        End Function
    End Class

    Module Module1

        Dim numbers() As Double = {1.5, .25, 3., 4.0F, 5.5f, 1.5J, 42, 42L, 7J, 0, 0L, 007, 0x1F, &HFF, &hff, &O17, &HZZ}
        Dim quote As String = "She said ""hello"" and left"
        Dim apostrophe As String = "it's not a comment"
        Dim unterminated As String = "this string never ends
        Dim empty As String = ""
        Dim unicode As String = "日本語 ✓ 😀"

        Sub Main(args As String())
            Dim items As New List(Of IStockItem)()
            Dim w As Widget = New Widget("W-001")
            AddHandler w.Changed, AddressOf OnChanged
            w.Quantity = 5

            For i As Integer = 0 To 10 Step 2
                If i Mod 3 = 0 AndAlso i <> 0 Then
                    Continue For
                ElseIf i >= 8 OrElse Not (i < 2) Then
                    Exit For
                Else
                    Console.WriteLine("i = {0}", i)
                End If
            Next

            For Each item In items
                Console.WriteLine(item.Sku)
            Next item

            Dim n = 0
            Do While n < 5
                n += 1
            Loop
            Do
                n -= 1
            Loop Until n <= 0

            While n < 3
                n *= 2 : n /= 1 : n ^= 1 : n &= 1
            End While

            Select Case n
                Case 0, 1
                    Console.WriteLine("small")
                Case Is > 100
                    Console.WriteLine("big")
                Case Else
                    Console.WriteLine("other")
            End Select

            Try
                Dim x = CInt("12") \ 5 + CDbl(3) ** 2 Mod 4
                Dim y = x << 2 >> 1 Xor 3 And 1 Or 2
                Dim b As Boolean = TypeOf w Is Widget AndAlso w IsNot Nothing
                Dim s = "abc" Like "a*"
                Dim ok = x = y Or x == y
            Catch ex As InvalidCastException When ex.Message <> ""
                Console.Error.WriteLine(ex.Message)
            Catch ex As Exception
                Throw
            Finally
                RemoveHandler w.Changed, AddressOf OnChanged
            End Try

            SyncLock items
                items.Clear()
            End SyncLock

            Using sr As New IO.StringReader("text")
                Console.WriteLine(sr.ReadToEnd())
            End Using

            With w
                .Quantity = 10
                Console.WriteLine(.Sku)
            End With

            Dim f As Func(Of Integer, Integer) = Function(v) v * 2
            Dim result = From it In items Where it.Quantity > 0 Select it.Sku
            Dim obj As Object = DirectCast(w, Object)
            Dim maybe = TryCast(obj, Widget)
            Dim arr(9) As Integer
            ReDim Preserve arr(19)
            Dim t = GetType(Widget)
            Dim ch As Char = "x"c
            Dim dt As Date = #1/1/2026#
            x .  y
            obj.
            Dim äpfel = 1
            GoTo done
done:
            Stop
        End Sub

        Private Sub OnChanged(sender As Object, e As EventArgs)
            Console.WriteLine("changed")	' tab before comment
        End Sub

        Declare Function GetTickCount Lib "kernel32" () As Integer
        Public Delegate Sub Notify(ByRef msg As String, Optional ByVal level As Integer = 1)
        Public Function Sum(ParamArray values() As Integer) As Integer
            Return values.Sum()
        End Function
        Public Shared Operator +(a As Money, b As Money) As Money
            Return a
        End Operator
    End Module
End Namespace
#End Region
' trailing comment