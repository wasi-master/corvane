-- Pig Latin ETL — traitement des journaux
/* multi-line comment
   spanning * several / lines
   ends here */
REGISTER 'lib/myudfs.jar';
DEFINE Tokenize org.apache.pig.builtin.TOKENIZE();
SET default_parallel 20;

raw = LOAD '/data/logs/2024-*' USING PigStorage('\t')
	AS (ts:chararray, user:chararray, bytes:long, ratio:double, tags:bag{t:tuple(tag:chararray)});

clean = FILTER raw BY bytes > 0 AND user IS NOT NULL AND ts MATCHES '2024-.*';
grouped = GROUP clean BY user PARALLEL 10;
stats = FOREACH grouped GENERATE
    group AS user,
    COUNT(clean) AS hits,
    SUM(clean.bytes) / 1024.0 AS kb,
    AVG(clean.ratio) AS avg_ratio,
    MAX(clean.bytes), flatten(group), group.$0;
ordered = ORDER stats BY hits DESC, kb ASC;
top10 = LIMIT ordered 10;
joined = JOIN stats BY user LEFT OUTER, profiles BY id USING 'replicated';
split_it = SPLIT joined INTO small IF hits < 100, big OTHERWISE;
casted = FOREACH raw GENERATE (int)bytes, (float)ratio, (map[])tags, (boolean)1;
x = FOREACH raw GENERATE REGEX_EXTRACT(ts, '(\\d+)-(\\d+)', 1), UPPER(user), lcfirst(user);
y = FOREACH raw GENERATE (bytes >= 10 ? 'big' : 'small'), bytes % 7, -bytes, 0x1F, 3.14e-2;
z = STREAM raw THROUGH `perl -ne 'print $_'`;
w = FILTER raw BY user != 'naïve ünïcödé ✓' OR user == "double";
v = FOREACH raw GENERATE user::name, @weird, #hash;
s = 'unterminated string
t = 'escaped at end \
continues'
STORE top10 INTO '/out/top10' USING PigStorage(',');
DUMP top10;
DESCRIBE stats;
ılong = 1;
/* comment opened at end of file
