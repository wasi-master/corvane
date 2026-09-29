# Puppet manifest — nœud de démonstration
import 'nodes/*.pp'

class nginx::install (
  String $version = 'latest',
  Boolean $manage_repo = true,
  $listen_port = 8080,
) inherits nginx::params {
  include stdlib
  include ::apt

  package { 'nginx':
    ensure  => $version,
    require => Apt::Source['nginx'],
  }

  file { '/etc/nginx/nginx.conf':
    ensure  => present,
    owner   => 'root',
    group   => 'root',
    mode    => '0644',
    content => template('nginx/nginx.conf.erb'),
    notify  => Service['nginx'],
  }

  service { 'nginx':
    ensure     => running,
    enable     => true,
    hasrestart => true,
  }
}

define nginx::vhost ($docroot, $port = 80, $aliases = []) {
  $conf = "/etc/nginx/sites-enabled/${name}.conf"
  $escaped = "cost: \$5 and ${port} or $port::x"
  file { $conf:
    ensure  => file,
    content => "server {\n  listen ${port};\n  root ${docroot};\n}\n",
  }
  exec { "reload-${name}":
    command     => '/usr/sbin/nginx -s reload',
    refreshonly => true,
    subscribe   => File[$conf],
  }
}

node 'web01.example.com', /^web\d+$/ {
  class { 'nginx::install':
    version => '1.25.3',
  }
  nginx::vhost { 'example.com':
    docroot => '/var/www/example',
    port    => 443,
  }
  @@user { 'deploy':
    ensure => present,
    uid    => 1001,
    shell  => '/bin/bash',
  }
  @sshkey { 'github.com':
    type => 'ssh-rsa',
  }
}

if $facts['os']['family'] == 'Debian' {
	$pkg = 'apache2'
} elsif $osfamily == 'RedHat' and $x != 3 {
	$pkg = 'httpd'
} else {
  fail("Unsupported OS: ${facts['os']['name']}")
}

case $::operatingsystem {
  'Ubuntu', 'Debian': { $svc = 'ssh' }
  default: { $svc = undef }
}

$multi = "first line
  still ${inside} the string
  and ends here"
$single = 'a
multi-line $notvar single'
$broken = $
$list = [1, 2, 300, 'x']
cron { 'backup':
  command => '/usr/local/bin/backup.sh',
  hour    => 2,
  minute  => absent,
}
ensure_resource('package', 'curl', {'ensure' => 'installed'})
Package <| tag == 'web' |> -> Service['nginx']
Zone
$unterminated = "never closed ${var}
$after = 'ok'
# ünïcödé comment — ✓ 😀 $x
$tail = "open at eof
