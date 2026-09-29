/* Scala sample /* nested comment */ still comment
 * with Ünïcödé: Ωmega, 日本
 */
package dev.corvane.sample

import scala.collection.mutable
import scala.concurrent.{Future, ExecutionContext}
import scala.util.{Try, Success, Failure}

@SerialVersionUID(1L)
sealed trait Shape extends Product with Serializable {
  def area: Double
}

final case class Circle(radius: Double) extends Shape {
  override def area: Double = math.Pi * radius * radius
}

case class Rect(w: Double, h: Double) extends Shape { def area = w * h }

object Model {
  val Hex = 0xFF
  val Big = 1000000L
  val Ratio = 2.5e-3
  val Half = .5f
  val sym = 'mySymbol
  val chr = 'x'
  val esc = '\n'
  val uni = 'é'
  val s = "plain \"escaped\" string"
  val multi = """triple "quoted" string
    spanning lines with \ backslash
    """
  val interp = s"Hello, $name and ${1 + 2}"
  val emptyTriple = """"""

  implicit val ec: ExecutionContext = ExecutionContext.global

  def describe(shape: Shape): String = shape match {
    case Circle(r) if r > 10 => s"big circle $r"
    case Circle(r) => "circle"
    case Rect(w, h) =>
      "rect"
    case _ => "unknown"
  }

  def sum(xs: List[Int]): Int = xs.foldLeft(0)(_ + _)

  lazy val cache = mutable.Map.empty[String, Int]

  def compute[A <: AnyVal, B >: Null](a: A)(implicit ev: A => Double): Option[Double] =
    Try(ev(a)) match {
      case Success(v) => Some(v)
      case Failure(e) => None
    }

  def ops(a: Int, b: Int): Boolean = a <= b && b >= a || a != b && !(a == b)
  val cons = 1 :: 2 :: Nil
  val tuple = (1, "two", 3.0)
  val fn: Int => Int = x => x * 2
  val hashOp = a ## b
  val annotated = (x: @unchecked) match { case _ => x }

  for {
    x <- List(1, 2, 3)
    if x % 2 == 1
    y <- Option(x)
  } yield x + y

  while (true) { println("loop"); return }
  do println("once") while (false)

  def forSomeExample(x: List[T] forSome { type T }) = x
  try { throw new Exception("boom") } catch { case e: Exception => () } finally { () }

  val unterminated = "no end
  val next = 1
}

abstract class Base[T](val value: T) {
  protected[this] var counter: Int = 0
  private lazy val name = getClass.getSimpleName
}

class Impl extends Base[String]("x") with Serializable {
  type Alias = Map[String, List[Int]]
  override def toString = s"Impl($value)"
}
