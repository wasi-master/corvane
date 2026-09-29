-- Schema for the jobs service
-- ünïcödé comment: 日本語
--no space needed after the dashes in generic SQL
# a hash is not a comment in generic SQL

/* A block comment
   spanning several lines
   /* nested */ still a comment
*/

/* one-line block */ SELECT 1;

CREATE TABLE IF NOT EXISTS "Jobs" (
	id         BIGINT PRIMARY KEY,
	name       VARCHAR(255) NOT NULL DEFAULT 'untitled',
	payload    BLOB,
	ratio      DECIMAL(10, 2),
	flags      BIT(8) DEFAULT b'00001111',
	mask       INT DEFAULT 0xFF,
	blob_hex   VARBINARY(16) DEFAULT X'DEADbeef',
	bits       INT DEFAULT 0b1011,
	created_at TIMESTAMP DEFAULT TIMESTAMP '2024-01-02 03:04:05',
	due        DATE DEFAULT DATE "2024-12-31",
	at_time    TIME,
	price      FLOAT DEFAULT 1.5e10,
	small      REAL DEFAULT 2.5E-3,
	whole      INTEGER DEFAULT 42,
	active     BOOLEAN DEFAULT TRUE,
	status     ENUM('queued', 'running', 'done') DEFAULT NULL,
	unsure     BOOL DEFAULT unknown
);

create index idx_jobs_name on Jobs (name);

INSERT INTO Jobs (id, name, payload) VALUES
	(1, 'it''s quoted', NULL),
	(2, 'escaped \' quote and \\ backslash', x''),
	(3, "double quoted string", B''),
	(4, 'ünïcödé ✓ 日本', 0x0);

SELECT j.id, j.name AS job_name, COUNT(*) AS total, j.ratio * 100 / 3 - 1 + 2 % 5
FROM Jobs j
LEFT JOIN runs r ON r.job_id = j.id AND r.status <> 'failed'
WHERE j.active = true
	AND j.name LIKE '%report%'
	AND j.id BETWEEN 10 AND 20
	AND j.id IN (1, 2, 3)
	AND NOT j.flags & 1 | 2 ^ 3 ~ 4
	OR j.name IS NOT NULL
	AND j.ratio >= .5
	AND j.ratio != 1.
	AND j.price <= 1e3
GROUP BY j.id, j.name
HAVING COUNT(*) > 1
ORDER BY total DESC, j.name ASC
LIMIT 10;

SELECT * FROM a.b.c WHERE x = ? AND y = ?;
SELECT ? , ?;
SELECT ?x FROM t;
SELECT t..col, `backtick`, @var, :named, $1, {d '2024-01-01'}, [bracketed];

UPDATE Jobs SET name = 'renamed', ratio = ratio * 1.1 WHERE id = 1;
DELETE FROM Jobs WHERE created_at < DATE '2020-01-01';
DROP TABLE old_jobs;
ALTER TABLE Jobs ADD COLUMN note TEXT;

BEGIN;
SELECT DISTINCT name FROM Jobs UNION SELECT name FROM archive;
select Count(*) from Jobs where Name = 'Mixed' And Id <> 7;

SELECT 'a string that
spans two lines' AS multi, "and a double one
too" AS other;

SELECT 'unterminated string
FROM nowhere;
' ;
/* unterminated comment at the end
still inside
