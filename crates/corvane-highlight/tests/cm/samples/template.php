<!DOCTYPE html>
<?php
  require_once __DIR__ . '/bootstrap.php';
  $title = $page['title'] ?? "Untitled";
  $items = get_items($db, limit: 20);
?>
<html lang="<?= $lang ?>">
<head>
  <meta charset="utf-8">
  <title><?php echo htmlspecialchars($title); ?> &middot; Site</title>
  <link rel="stylesheet" href="<?= asset('app.css') ?>">
  <style>
    body { color: <?= $theme['fg'] ?>; background: #fff; }
    .item:hover { opacity: .8 }
  </style>
  <script>
    window.CONFIG = <?= json_encode($config, JSON_PRETTY_PRINT) ?>;
    const user = "<?= addslashes($user->name) ?>";
    if (user.length > 0) { console.log(`Hello ${user}`); }
  </script>
</head>
<body class="<?php if ($dark): ?>dark<?php else: ?>light<?php endif; ?>">
  <?php if (count($items) > 0): ?>
    <ul id="items">
      <?php foreach ($items as $i => $item): ?>
        <li class='item <?= $i % 2 ? "odd" : "even" ?>' data-json='<?= json_encode($item) ?>'>
          <a href="/items/<?= urlencode($item['slug']) ?>" title="<?= $item['title'] ?>">
            <?= $item['title'] ?> — <?= number_format($item['price'], 2) ?> €
          </a>
        </li>
      <?php endforeach; ?>
    </ul>
  <?php else: ?>
    <p class="empty">Nothing here 🤷</p>
  <?php endif ?>

  <?php
  // a PHP block with braces left open across HTML
  function render_card(array $card) {
  ?>
    <div class="card">
      <h2><?= $card['title'] ?></h2>
      <p><?= nl2br(e($card['body'])) ?></p>
    </div>
  <?php
  }

  $cards = array_map(fn($c) => ['title' => ucfirst($c), 'body' => "Body of $c"], ['a', 'b']);
  foreach ($cards as $card) { render_card($card); }
  # comment ending the block ?>
  <footer>
    <p>&copy; <?= date('Y') ?> Example Inc. <?php // trailing comment ?> All rights reserved.</p>
    <?php /* block comment with ?> inside */ ?>
    <p>After the block comment.</p>
    <?php $text = <<<HTML
      <strong>$title</strong> at {$now->format('H:i')}
      HTML;
    echo $text; ?>
    <input value="<?= $value ?>" <?= $disabled ? 'disabled' : '' ?>>
    <img src="<?=$src?>" alt='<?=$alt?>'>
    <a title="<?= "quoted" ?>" href='<?= '/x' ?>'>quote-pending attributes</a>
    <p>Text then <?php echo $inline; ?> then more text <?php if ($x) { ?>
      <b>inside an open brace</b>
    <?php } ?></p>
    <div data-a="<?php echo "multi" ?>
      line" data-b=<?= $unquoted ?>>
    </div>
  </footer>
  <?php
    $unterminated = "this string
      keeps going ?> not the end
      until here";
    $obj = new class { public $x = 1; };
    echo $obj->x, TRUE, NULL, __LINE__;
  ?>
  <script type="text/javascript">
    var items = <?php echo count($items) ?>;
  </script>
  <?xml-ish processing ?>
  <?
    // short open tag
    $short = true;
  ?>
</body>
</html>
<?php
// closing tag omitted at the end of file
$final = 'done';
