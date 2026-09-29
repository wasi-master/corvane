<html><body>
<p title="<?php echo "hi" ?>" class="x">Attr cut at a PHP tag with a double-quoted string</p>
<p title='<?php echo 'x' ?>' data-a="<?= $a ?>b<?= $c ?>">Two tags in one attribute</p>
<p><?php if ($cond) { ?>kept state<?php } else { ?>other<?php } ?></p>
<div <?php echo $attrs ?> id="after-attrs">bare php in a tag</div>
<script>
	var a = <?= $a ?>, b = "<?php echo $b; ?>", c = '<?= $c ?>';
	var cmp = x <? echo 1 ?>;
	/* js comment <?php echo "in comment" ?> still comment */
</script>
<style>
	.a { width: <?php echo $w ?>px; color: "<?= $c ?>"; }
</style>
<!-- html comment <?php echo "in html comment" ?> end -->
<?php
$shift = $a << 2 >> 1;
$cmp = $a <=> $b;
$lt = $x < $y && $y<$z;
$notHeredoc = <<< ;
$weird = <<<"" ;
$str = "A string with ?> inside does not close PHP";
$str2 = 'single with ?> too';
$nested = "Deep {$a[$b['c']]} and {$obj->method(1, "inner")} done";
$braces = "{$a} {$b}{$c} {{$d}} { $e } {\$f}";
$dollar = "Price: $ 5, $$var, $1, ${expr}, $obj->";
$idx = "$a[0x1F] $a[-1] $a[b c] $a->$b $a->1";
$doc = <<<EOT
    Inline EOT closes early: EOT and the rest
EOT;
$doc2 = <<<  LABEL
  spaced label $var {$arr['k']}
  LABEL;
$now = <<<'NOW'
  {$x} $y ${z}
  NOW;
$num = 1.5e-3 + 0XFF + 017 + 0b11 + 10_000.5;
$cast = (string) $x . (bool) $y;
$ref = &$arr['key'];
$static = static::$instance ?? self::create();
$null = null ?? NULL ?? Null;
$bool = true || FALSE || True;
$magic = __DIR__ . __CLASS__ . __METHOD__ . __NAMESPACE__;
function &getRef(array &$a, ...$rest): ?int { return $a[0]; }
abstract class Base { abstract public function run(): void; }
$anon = new class(10) extends Base implements Countable {
	public function __construct(public int $n) {}
	public function run(): void { yield from gen(); }
	public function count(): int { return $this->n; }
};
try { throw new \RuntimeException("fail: {$e->getMessage()}"); }
catch (\LogicException | \RuntimeException $e) { error_log($e); }
finally { unset($e); }
goto end;
end:
list('a' => $x, 'b' => $y) = $pair;
[$p, [$q, $r]] = $nested;
$html = "<div class=\"$cls\">" . htmlspecialchars($t, ENT_QUOTES) . '</div>';
echo <<<HTML
<p>$name</p>
HTML;
switch ($x) { case 1: case 2: break; default: echo 'x'; }
while (--$i) { do { $j++; } while ($j < 10); }
for ($i = 0; $i < 10; $i++): endfor;
declare(ticks=1);
if ($a) { if ($b) { ?>
	<b>inside two braces</b>
<?php } } ?>
<p>ünïcødé — 日本語 — emoji 🎉 <?= "ünï $x" ?></p>
<?php
	$tab	=	"tabs	inside";
	// comment at end of file
