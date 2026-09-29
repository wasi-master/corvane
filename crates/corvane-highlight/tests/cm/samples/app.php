<?php
declare(strict_types=1);

namespace App\Http\Controllers;

use App\Models\{User, Repository};
use Illuminate\Support\Facades\DB;
use function array_map as map;

/**
 * Repository controller.
 *
 * @package App
 */
#[Route('/repos', methods: ['GET'])]
final class RepositoryController extends Controller implements \JsonSerializable
{
    public const VERSION = '1.2.3';
    private static ?self $instance = null;
    protected array $cache = [];

    public function __construct(
        private readonly UserService $users,
        protected int $limit = 25,
    ) {
        parent::__construct();
    }

    // line comment: ? > spaced out is not a close tag
    public function index(Request $request): array|null
    {
        $name = $request->input('name', "default");
        $count = count($this->cache) + 0x1F + 0b1010 + 1_000_000 + 3.14e-2;
        $greeting = "Hello, $name! You have {$count} repos and ${count} stars.";
        $nested = "Item: $items[0] and $items[key] and $items[$idx] and $user->name.";
        $complex = "Deep: {$user->profile['avatar']} and {$obj->method()}";
        $escaped = "Tab\tnewline\n dollar \$notvar quote \" done";
        $single = 'No $interpolation {$here} \' escaped';
        $multi = "spans
            several $lines
            of text";
        $heredoc = <<<EOT
            Dear $name,
            Your total is {$order->total()} for {$count} items.
            EOT;
        $nowdoc = <<<'SQL'
            SELECT * FROM users WHERE name = '$name' AND id = {$id}
            SQL;
        $quotedHeredoc = <<<"HTML"
          <div class="$cls">Hi</div>
          HTML;
        $shifted = 1 << 3;
        $spaceship = $a <=> $b;
        $nullsafe = $user?->profile?->bio ?? 'none';
        $fn = fn(int $x): int => $x * 2;
        $closure = function ($item) use ($limit, &$total): bool {
            return $item->size < $limit;
        };
        $result = match (true) {
            $count > 100 => 'many',
            $count > 10, $count > 5 => 'some',
            default => throw new \InvalidArgumentException("bad: {$count}"),
        };
        # hash comment
        /* block
           comment */
        if (isset($this->cache[$name]) && !empty($name)) {
            echo htmlspecialchars($name), PHP_EOL;
        } elseif ($name === null || $name == FALSE) {
            print_r(array_filter($this->cache, fn($v) => $v !== null));
        } else {
            foreach ($users as $key => $user): ?>
                <li data-id="<?= $user->id ?>"><?= e($user->name) ?></li>
            <?php endforeach;
        }
        try {
            DB::transaction(static fn() => User::query()->where('active', true)->update(['seen' => now()]));
        } catch (\Throwable $e) {
            error_log($e->getMessage());
        } finally {
            $this->limit--;
        }
        return ['name' => $name, 'count' => $count, 'emoji' => "🚀 $name 🎉", '__CLASS__' => __CLASS__];
    }

    enum Status: string {
        case Active = 'active';
        case Archived = 'archived';
    }

    abstract protected function handle(object $event, mixed ...$args): static;
}

trait Loggable { public function log(string $msg): void { echo "[LOG] {$msg}\n"; } }

interface HasName { public function name(): string; }

$repo = new RepositoryController(new UserService(), limit: 10);
list($a, $b) = [1, 2];
[$c, [$d, $e]] = [3, [4, 5]];
goto end;
end:
exit(0);
?>
<p>Trailing HTML after the closing tag</p>
