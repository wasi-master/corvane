// Cypher sample — requêtes Neo4j
CREATE CONSTRAINT person_name IF NOT EXISTS FOR (p:Person) REQUIRE p.name IS UNIQUE;
CREATE INDEX movie_title FOR (m:Movie) ON (m.title);

MATCH (tom:Person {name: "Tom Hanks"})-[:ACTED_IN]->(movie:Movie)
WHERE movie.released >= 1990 AND movie.released < 2000
RETURN tom.name AS actor, collect(movie.title) AS titles, count(*) AS n
ORDER BY n DESC
SKIP 5 LIMIT 10;

MATCH p = shortestPath((a:Person)-[*..15]-(b:Person))
WHERE a.name = 'Kevin Bacon' AND b.name <> 'Meg Ryan'
RETURN length(p), [x IN nodes(p) | x.name] AS names;

OPTIONAL MATCH (n)-[r:KNOWS|FOLLOWS]->(m)
WITH n, size((n)--()) AS degree, toFloat(r.weight) * 2.5 AS w
WHERE degree > 3 XOR w <= 0.5 OR n:Admin
	SET n.score = coalesce(n.score, 0) + 1, n:Ranked
REMOVE n.temp
RETURN DISTINCT n, CASE WHEN w > 1 THEN 'heavy' ELSE 'light' END;

UNWIND range(1, 100) AS i
MERGE (u:User {id: i})
ON CREATE SET u.created = timestamp()
ON MATCH SET u.seen = u.seen + 1;

LOAD CSV WITH HEADERS FROM 'file:///people.csv' AS row FIELDTERMINATOR ';'
CALL {
  WITH row
  CREATE (:Person {name: row.name, née: "Zoë Ünïcödé ✓"})
} IN TRANSACTIONS OF 500 ROWS;

CALL db.labels() YIELD label
RETURN label STARTS WITH 'P' AS p, label ENDS WITH 'n', label CONTAINS "rs";

FOREACH (x IN [1, 2, 3] | CREATE (:Num {v: x}));
MATCH (a)-[r]->(b) WHERE all(k IN keys(r) WHERE k =~ '.*id') DETACH DELETE a;
SHOW DATABASES YIELD name, currentStatus;
GRANT TRAVERSE ON GRAPH neo4j NODES Person TO reader;
EXPLAIN PROFILE MATCH (n:`Weird Label`) RETURN n.`prop name`, $param, $0;
RETURN "unterminated string, and more
RETURN 'another open
RETURN 1 / 2 // trailing comment
RETURN reduce(acc = 0, x IN [1,2] | acc + x) % 3 ^ 2, -1.5e3, pi(), e();
MATCH (n) RETURN n.name <=> 'x', n.a !== 'y', label:with-dash;
// final comment with emoji 😀
