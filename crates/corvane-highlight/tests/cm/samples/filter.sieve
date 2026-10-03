# Sieve filter — règles de tri du courrier
require ["fileinto", "reject", "vacation", "envelope", "imap4flags", "variables"];

/* block comment
   over several lines */
if header :contains "subject" ["[SPAM]", "***SPAM***"] {
	fileinto "Junk";
	stop;
}

if allof (address :is :domain "from" "example.com",
          not exists "list-id",
          size :over 100K) {
    addflag "\\Flagged";
    fileinto :copy "INBOX/Work";
} elsif anyof (header :matches "to" "*@lists.example.org", envelope :all "from" "bot@*") {
    fileinto "Lists";
} else {
    keep;
}

if size :under 2M { keep; }
if size :over 1G { discard; }
if true { set "name" "Zoë Ünïcödé ✓"; }
if false { redirect "archive@example.com"; }

vacation :days 7 :subject "Out of office" text: # comment on the text: line
  I am away until Monday.
  Your message about "${subject}" will be read later.
..dot-stuffed line
.

reject text:
Sorry, we do not accept attachments.
.
setflag "a \"quoted\" flag";
set "multi" "string with trailing backslash \
continued on next line";
if header :regex "subject" "^[0-9]{3}$" { fileinto "Numbers"; }
if header :is "x-priority" ["1", "2"] { addflag "urgent"; }
if not header :contains "x-mailer" "Outlook" { removeflag "\\Seen"; }
if exists ["x-spam-flag", "x-virus"] { fileinto :create "Quarantine"; stop; }
if address :localpart :is "to" "billing" { fileinto "Finance/Invoices"; }
if envelope :domain :is "to" "example.net" { redirect :copy "ops@example.net"; }
if string :matches "${subject}" "*urgent*" { setflag "\\Flagged"; }
if body :text :contains ["unsubscribe", "opt-out"] { fileinto "Newsletters"; }
if date :value "ge" :originalzone "date" "hour" "18" { fileinto "After-Hours"; }
if currentdate :zone "+0100" :value "lt" "weekday" "6" { keep; } else { discard; }
if header :count "ge" :comparator "i;ascii-numeric" "received" "10" { reject "loop"; }
@unknown _under_score 42k 7x
:_x1 :9 text : text
/* unterminated comment at eof
