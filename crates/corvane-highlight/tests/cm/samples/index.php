<?php
declare(strict_types=1);

namespace App\Http\Controllers;

use App\Models\{User, Post};
use Illuminate\Support\Facades\DB;

/**
 * Renders the dashboard.
 * Multi-line doc comment with <b>tags</b> and ?> not closing it.
 */
#[Route('/dashboard', methods: ['GET'])]
final class DashboardController extends Controller implements Renderable
{
	public const VERSION = "2.1";
	private static ?array $cache = null;
	protected readonly int $limit;

	public function __construct(private UserRepository $users, int $limit = 0x1F)
	{
		$this->limit = $limit ?: 25;
		parent::__construct();
	}

	public function index(Request $request): Response
	{
		$name = $request->input('name', 'Gäst');
		$items = [1, 2.5, 1e3, 0b101, 0o17, 1_000_000, .5, -3];
		$user = $this->users->find((int) $request->id) ?? new User();
		// single-line comment with a URL: https://example.com/?q=1
		$single = 'It\'s a single-quoted $string with {$no} interpolation';
		$double = "Hello, $name! You have {$user->count} new {$items[0]} items";
		$complex = "Value: ${name} and {$user->profile->email} and $items[1] and $user->name.";
		$arr = "Key $matrix[0][1] and $map[key] and $map[$idx] and $obj->prop->deep";
		$escaped = "Tab\there \$notvar \"quoted\" \\ backslash {\$literal}";
		$multi = "first line
			second line with $name
			third line";
		$heredoc = <<<EOT
			Dear $name,
			Your balance is {$user->balance} as of {$date->format('Y-m-d')}.
			Array access: $items[2] and method: {$this->limit}
			EOT;
		$quotedHeredoc = <<<"HTML"
			<div class="$name">{$user->id}</div>
			HTML;
		$nowdoc = <<<'SQL'
			SELECT * FROM users WHERE name = '$name' AND id = {$id}
			SQL;
		# hash comment
		match (true) {
			$user instanceof Admin => $this->admin($user),
			default => null,
		};
		foreach ($items as $key => &$value) {
			if ($value > 10 && !is_null($key) || $value <=> 3) { continue; }
			$value *= 2; $value .= "!"; $value **= 2;
		}
		$fn = fn($x) => $x * 2;
		$closure = function () use ($name) { return strtoupper($name); };
		echo json_encode(['ok' => TRUE, 'file' => __FILE__, 'line' => __LINE__]);
		print_r(array_map($fn, array_filter($items)));
		$sql = DB::table('users')->where('active', 1)->get();
		return new Response(view('dashboard', compact('user', 'items')));
	}
}

enum Status: string { case Active = 'active'; case Banned = 'banned'; }
interface Renderable { public function render(): string; }
trait Loggable { abstract protected function log(string $msg): void; }
?>
<!DOCTYPE html>
<html lang="<?= htmlspecialchars($lang) ?>">
<head>
	<title><?php echo $title ?? 'Untitled'; ?></title>
	<style>
		body { color: <?= $color ?>; margin: 0 }
	</style>
	<script>
		const user = <?= json_encode($user) ?>;
		const items = "<?php echo implode(',', $items) ?>";
		if (user.id < 10) { console.log("small"); }
	</script>
</head>
<body class="<?php echo $bodyClass; ?> page">
	<a href="/u/<?= $user->id ?>" title="<?php echo "Profile of $name" ?>">Profile</a>
	<a title='<?php echo 'single' ?>'>Single</a>
	<?php if ($user->isAdmin()): ?>
		<p>Welcome back, <strong><?= $user->name ?></strong> — ünïcødé 日本</p>
	<?php elseif ($user->isGuest()): ?>
		<p>Hello guest</p>
	<?php else: ?>
		<p>Nothing</p>
	<?php endif; ?>
	<ul>
	<?php foreach ($items as $i => $item) { ?>
		<li data-index="<?= $i ?>"><?= $item ?></li>
	<?php } ?>
	</ul>
	<?php
		/* block comment
		   spanning lines */
		$footer = sprintf("%s &copy; %d", "Corvane", date('Y'));
	?>
	<footer><?= $footer ?></footer>
	<? echo "short open tag"; ?>
	<p>Unclosed php at the end:</p> <?php // line comment closes ?> <b>after</b> <?php # hash too ?><i>x</i>
<?php
	$x = "unterminated string
	continues on the next line
