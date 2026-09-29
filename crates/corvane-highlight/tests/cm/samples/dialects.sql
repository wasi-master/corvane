-- dialect torture test
--no space comment
# hash comment
// slash comment
/* block /* nested */ still */ after
SELECT `col``umn`, `unterminated
SELECT "ident""quoted", "unterminated ident
SELECT @var, @@session.sql_mode, @@global.x, @'quoted var', @"dq var", @`bt var`, @, :named, $1, ?1, ? , ?;
SELECT \N, \g, \G, \., \x, \
SELECT N'national', n"dq", _utf8'charset', _latin1"x", _utf8 notcast, E'esc \' quote', e"dq", E x;
SELECT x'FF', X'', b'0101', B'', 0b101, 0xDEAD, 0XBEEF, 12, 1.5, 1., 1..2, .5, .5e3, .e, ..x, .name, .#x, 3e+5, 2E-2;
SELECT {d '2024-01-01'}, { ts "2024-01-01 00:00:00" }, {t'x'}, {bogus}, [bracket], (paren);
SELECT DATE '2024-01-01', time "10:00", timestamp  '2024', datetime '2024', datetimeoffset '1', zone 'utc', date;
SELECT a || b, a::int, a -> b, a ->> 'k', a @> b, a <@ b, a ~* b, a !~ b, a # b, a ?| b, a <=> b, a := 1, x / y, x % y;
SELECT 'it''s', 'back\'slash', 'tail\\', "dq \" esc";
CREATE TRIGGER trg BEFORE INSERT ON t FOR EACH ROW BEGIN DECLARE x INT; END;
BEGIN TRANSACTION; COMMIT; ROLLBACK; GO
select nolock, holdlock, isnull(a, 0), newid(), object_id('t'), rowcount_big();
SELECT CAST(a AS VARCHAR2(10)), NVL(a, 0), SYSDATE, ROWNUM FROM dual;
SELECT pragma, vacuum, glob, autoincrement, without, rowid;
SELECT infinity, NaN, nan, ttl, writetime, keyspace, frozen, timeuuid;
SELECT jsonb, bytea, serial, ilike, returning, lateral, tablesample, window;
SELECT explode, array_contains, approx_percentile, map_keys, st_area, try_cast;
SELECT unknown, true, false, null, TRUE, Null, current_date, current_timestamp;
source file.sql; charset utf8; exit; quit; help; status; tee out.txt;
SELECT ünïcödé, 日本, ßtraße, Kelvin;
SELECT 'unterminated string
continues here' done;
SELECT E'unterminated escape
still';
/* unterminated
