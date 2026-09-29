package Config::Reader;
# Reads the key/value pairs stored after __DATA__.
use strict; use warnings;

my %defaults;
while (my $row = <DATA>) {
    chomp($row);
    next if $row =~ m/^#/;
    my ($k, $v) = split /\s*=\s*/, $row, 2;
    $defaults{lc $k} = $v // '';
}
printf "%-10s %s\n", $_, $defaults{$_} for sort keys %defaults;
my @stack = (1, 2, 3); push @stack, 4; pop @stack; shift(@stack);
my $s = sprintf('%05.2f', 3.14159);
print "ok\n" if exists $defaults{name} and defined $defaults{port};
local $/ = undef;
local $\ = "\n";
my $sep = $/;
return wantarray ? @stack : \@stack;
__DATA__
name = corvane
port = 8080
# comment inside data
path = /usr/local/lib/perl5
