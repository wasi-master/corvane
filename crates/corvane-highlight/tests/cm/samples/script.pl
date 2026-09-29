#!/usr/bin/perl
# Deploy helper — reads a manifest and syncs files. Ünïcödé comment ✓
use strict;
use warnings;
use utf8;
use File::Basename qw(basename dirname);
use Getopt::Long;
use POSIX ":sys_wait_h";
use Data::Dumper;
use constant { MAX_RETRIES => 3, TIMEOUT => 30.5 };

our $VERSION = '1.04';
my ($verbose, $dry_run, @targets, %opts) = (0, 0);
my $count = 0x1F_FF + 0b1010 + 1_000_000 + 3.14e-2 + .5 + 12. + -7;
my @numbers = (1..10);
local $| = 1;
$, = ", ";
$" = "-";
$; = "|";

GetOptions(
    'verbose|v' => \$verbose,
    'dry-run'   => \$dry_run,
    'target=s@' => \@targets,
) or die "usage: $0 [--verbose] [--dry-run] --target DIR\n";

sub log_msg {
    my ($level, @msg) = @_;
    return unless $verbose or $level eq 'error';
    printf STDERR "[%s] %s\n", uc $level, join(' ', @msg);
}

sub parse_manifest {
    my $file = shift;
    my %seen;
    open(my $fh, '<:encoding(UTF-8)', $file) or die "can't open $file: $!";
    while (my $line = <$fh>) {
        chomp $line;
        next if $line =~ /^\s*(#|$)/;
        $line =~ s/^\s+|\s+$//g;
        $line =~ s{\\}{/}g;
        $line =~ s(\.bak)(.orig)i;
        $line =~ s<foo><bar>;
        $line =~ s[a+][b]gex;
        $line =~ s!old!new!;
        $line =~ s|x|y|;
        $line =~ tr/a-z/A-Z/;
        $line =~ tr{a-f}{0-5};
        $line =~ y/,/;/;
        (my $copy = $line) =~ s/\t/    /g;
        if ($line =~ m{^(\w+)\s*=\s*(.*)$}x) {
            $seen{$1} = $2;
        } elsif ($line =~ m/^\[(.+?)\]$/) {
            log_msg('debug', "section $1");
        } elsif ($line =~ m(^include\s+(\S+))) {
            parse_manifest($1);
        } elsif ($line =~ m<^@@(.*)$>ms) {
            warn "odd line: $&";
        } elsif ($line =~ m!bang!) {
            1;
        } elsif ($line !~ /\S/) {
            last;
        }
    }
    close $fh;
    return \%seen;
}

my @words = qw(alpha beta gamma delta);
my @more  = qw/one two three/;
my @brace = qw{ x y z };
my @angle = qw<p q>;
my @sq    = qw[ i j ];
my $single = q(it's a (nested) string);
my $braced = q{curly};
my $bang   = q!bang!;
my $double = qq{Hello, $ENV{USER}! Today is @{[ scalar localtime ]}};
my $dq2    = qq(paren "quotes");
my $dq3    = qq[square];
my $dq4    = qq<angle>;
my $dq5    = qq/slash $count/;
my $dq6    = qq~tilde~;
my $re     = qr/^\d{3}-\d{4}$/i;
my $re2    = qr{(?<year>\d{4})-(?<mon>\d\d)}x;
my $re3    = qr(paren);
my $re4    = qr!bang!msix;
my $out    = qx(ls -la /tmp);
my $out2   = qx{uname -a};
my $out3   = qx/date/;
my $out4   = `hostname`;
my $out5   = qx[whoami];
my $out6   = qx<id>;

my $str = "escaped \"quote\" and \\ backslash and \$dollar and @array";
my $sq  = 'single \'quoted\' with $no @interp';
my $multi = "this string
spans several
lines";
my $multi_q = q{this one
also spans};

print <<EOF;
Heredoc body with $count and @numbers
  indented line
EOF

print <<"END_TEXT";
Quoted heredoc: $VERSION
END_TEXT

print <<'RAW';
No $interpolation here @at all
RAW

my $indented = <<~EOT;
    tilde heredoc
    EOT

my %config = (
    name    => 'deploy',
    retries => MAX_RETRIES,
    timeout => TIMEOUT,
    list    => [1, 2, 3],
    nested  => { a => 1, b => [qw(x y)] },
);
my $name = $config{name};
my $first = $config{list}->[0];
my $last_ix = $#numbers;
my $last_el = $numbers[$#numbers];
my $len = scalar(@numbers);
my @sorted = sort { $a <=> $b } @numbers;
my @rsorted = reverse sort { lc($a) cmp lc($b) } @words;
my @mapped = map { $_ * 2 } grep { $_ % 2 == 0 } @numbers;
my %inverted = reverse %config;
my @slice = @numbers[2..4];
my @hslice = @config{qw(name retries)};
my $ref = \@numbers;
my @deref = @$ref;
my @deref2 = @{$ref};
my $code = sub { return $_[0] ** 2 };
my $res = $code->(4);
my $obj = Some::Class->new(name => 'x');
$obj->method(1, 2)->chain;
Some::Package::function();
my $pkg = __PACKAGE__;
my $file = __FILE__ . ':' . __LINE__;

if ($count > 10 && $name ne 'test' || !defined $first) {
    $count += 1;
    $count -= 2;
    $count *= 3;
    $count /= 4;
    $count **= 2;
    $count %= 7;
    $count .= "x";
    $count ||= 0;
    $count //= 5;
    $count x= 2;
} elsif ($count == 0 and not $dry_run) {
    $count++;
    --$count;
} else {
    unless ($count <= 5) { $count = $count >= 1 ? 1 : 0 }
}

foreach my $t (@targets) {
    next unless -d $t;
    for (my $i = 0; $i < MAX_RETRIES; $i++) {
        my $rc = system('rsync', '-a', $t, '/srv/');
        last if $rc == 0;
        sleep 2 ** $i;
    }
}

my $i = 0;
while ($i < 5) { $i++ } continue { print "loop $i\n" }
until ($i == 0) { $i-- }
do { $i++ } while ($i < 3);

my $pid = fork();
if (!defined $pid) {
    die "fork failed: $!";
} elsif ($pid == 0) {
    exec('/bin/true') or exit 1;
}
waitpid($pid, WNOHANG);
my $status = $? >> 8;

eval {
    die { code => 500, message => "boom" };
    1;
} or do {
    my $err = $@ || 'unknown';
    warn "caught: $err->{message}\n" if ref $err;
};

local $_ = "The quick brown fox";
my @parts = split /\s+/;
my $joined = join ',', @parts;
my $matched = $_ =~ /quick/ ? $1 : undef;
my ($x, $y) = /(\w+)\s+(\w+)/;
my $pos = index($_, 'brown');
my $sub = substr $_, 4, 5;
print "match: $1, pre: $`, post: $'\n" if /brown/;
print "pid $$ prog $0 args @ARGV time $^T os $^O\n";
print STDOUT "perl $] version $^V\n";
print "dollar-hash: $#{$ref}\n";
my $total = @numbers + 0;
my $avg = $total / scalar @numbers;
my $ratio = $avg / 2 / 3;
my $rx_after = $str =~ /foo/g;
my $spaced = $str =~   /bar/;
my @globs = <*.txt>;
my $line = <STDIN>;
my %h; $h{key} = 1; $h{ key2 } = 2; $h{'k3'} = 3;
my $bareword_key = $h{BEGIN};

package My::Module;
use parent -norequire, 'Base::Class';

sub new {
    my ($class, %args) = @_;
    my $self = bless { %args }, $class;
    return $self;
}

sub AUTOLOAD { our $AUTOLOAD; return if $AUTOLOAD =~ /DESTROY/ }
sub DESTROY { }

BEGIN { unshift @INC, './lib' }
END { print "done\n" }

format STDOUT =
@<<<<<<<<< @>>>>> @||||||
$name,     $count, $total
.

my $unterminated = "open string at end of line
continues here";
my $weird = $h{x} . q
(split across) . 'ok';
my $tabbed	=	"tabs	inside";
my $emoji = "héllo wörld — 日本語 🚀";
my $s_after_paren = (1) s/x/y/;
my @eol_words = qw(
    spread over lines
);
my $eol_q = q(
text);
my $eol_qq = qq/
more/;
my $eol_m = $str =~ m
/x/;
$str =~ s{
  multi
}
{replacement}gx;
$str =~ s/unterminated
still pattern/repl/;
my $ctrl = $^W + $^X . ${^MATCH} . ${10} . $10 . @{ $ref } . %$ref;
my $qxyz = qxyz + qqq + qwerty + mm + sy + try + yes;
my $amp = &do_it(1) & 3;
my $label = LABEL: while (1) { last LABEL }
my $A = MY_CONST + Foo::BAR + FOO9 + ABc;
print {$fh} "to handle\n";
my $tr_after = split /,/, $joined;

=item Some pod item

This is documentation text with $vars and code-looking things;
    print "not code";

=cut

my $after_pod = 42;

=pod

Plain pod is not recognised by this mode.

=cut

1;
__END__
Everything after __END__ is a comment: my $x = "not code";
=head1 NAME
