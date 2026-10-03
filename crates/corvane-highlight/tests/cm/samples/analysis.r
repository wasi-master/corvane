# Survival analysis of deployment latencies — ünïcödé comment ✓
library(dplyr)
library("ggplot2")
suppressPackageStartupMessages(require(tidyr))

# ---- constants ----
MAX_ITER <- 1000L
tolerance = 1e-8
ratio <- .5 + 0.25 + 3. + 1.5e+3 + 2e-4 + .1e2 + 0x1F + 0xABCdef + 42L
flags <- c(TRUE, FALSE, NA, NULL, Inf, -Inf, NaN)
missing <- c(NA_integer_, NA_real_, NA_complex_, NA_character_)
`odd name` <- 5
`unterminated backtick
x <<- 10
20 -> y
30 ->> z
w = 40
v <- -1

# ---- strings ----
s1 <- "double quoted with \"escapes\" and \\ backslash"
s2 <- 'single quoted with \'escape\''
s3 <- "tab\there\nnewline \x41 é \u{1F600} \U0001F600 \U{1F680} \101 \7"
s4 <- "multi-line string
continues on the next line
and ends here"
s5 <- r"(raw strings are not special here)"
s6 <- "héllo wörld — 日本語 🚀"
s7 <- 'unterminated \u{oops'
s8 <- "trailing backslash \
still string"

# ---- functions ----
summarise_latency <- function(df, col = "latency_ms", probs = c(0.5, 0.9, 0.99), ...) {
  stopifnot(is.data.frame(df), col %in% names(df))
  vals <- df[[col]]
  vals <- vals[!is.na(vals) & vals >= 0]
  if (length(vals) == 0) {
    return(list(n = 0, mean = NA))
  } else if (length(vals) < 10) {
    warning("few observations: ", length(vals))
  } else {
    message(sprintf("%d observations", length(vals)))
  }
  q <- quantile(vals, probs = probs, na.rm = TRUE)
  list(n = length(vals), mean = mean(vals), sd = sd(vals), q = q)
}

fit_model <- function(formula, data,
                      family = binomial(link = "logit"),
                      weights = NULL) {
  model <- glm(formula, data = data, family = family, weights = weights)
  structure(list(model = model, call = match.call()), class = "fit")
}

print.fit <- function(x, ...) {
  cat("Call:\n"); print(x$call)
  invisible(x)
}

# ---- control flow ----
for (i in seq_len(10)) {
  if (i %% 2 == 0) next
  if (i > 7) break
  total <- total + i
}

k <- 0
while (k < 5) k <- k + 1
repeat {
  k <- k - 1
  if (k <= 0) break
}

result <- if (k == 0) "zero" else "non-zero"
if (x > 1)
  y <- 2 else
  y <- 3
for (j in 1:3) print(j); print("after")

f <- function(a, b) a + b
g <- function(x) {
  x ^ 2 ** 3
}
h <- \(x) x * 2

# ---- pipes, operators, indexing ----
clean <- raw_data %>%
  filter(!is.na(value), status != "failed") %>%
  mutate(value_log = log1p(value),
         bucket = cut(value, breaks = c(-Inf, 0, 10, Inf))) %>%
  group_by(region, bucket) %>%
  summarise(n = n(), avg = mean(value)) |>
  arrange(desc(n))

m <- matrix(1:12, nrow = 3, byrow = TRUE)
m2 <- m %*% t(m)
inter <- a %in% b
mod <- 17 %/% 5 + 17 %% 5
div <- 10 / 3
cmp <- a != b && c <= d || !e
bits <- xor(TRUE, FALSE) | (1 & 0)
form <- y ~ x1 + x2:x3 - 1
ns <- stats::median(1:10) + base:::`+`(1, 2)
lst <- list(a = 1, b = "two", c = list(d = 3))
lst$a; lst$c$d; lst[["b"]]; lst[1:2]
df$new_col <- df$old_col * 2
df[df$x > 0, c("x", "y")]
env <- new.env(); assign("key", 1, envir = env)
e <- quote(x + y); eval(e); bquote(.(x) + 1)
p <- parse(text = "1 + 2"); deparse(p)
dots <- function(...) ..1 + ..2
spread <- ...length()
.hidden <- 1
.Machine$double.eps
x.y.z <- 3.14
f2 <- function(x) { x[1]; x[[2]]; x@slot }

# ---- apply family and closures ----
squares <- sapply(1:10, function(n) n^2)
pairs <- mapply(function(a, b) paste(a, b, sep = "-"), letters[1:3], LETTERS[1:3])
counter <- local({
  count <- 0
  function() {
    count <<- count + 1
    count
  }
})
tryCatch({
  stop("boom")
}, error = function(e) {
  message("caught: ", conditionMessage(e))
}, finally = cat("done\n"))

call_args <- f(
  alpha = 1,
  beta = 2
)
nested <- outer(inner(a = 1), b = 2)
	tabbed	<-	"tabs"
emoji <- "🚀"; ok <- TRUE
