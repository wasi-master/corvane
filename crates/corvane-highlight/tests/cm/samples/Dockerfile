# syntax=docker/dockerfile:1.7
# A multi-stage build — ünïcödé comment 日本語

ARG GO_VERSION=1.22
ARG BASE_IMAGE="alpine:3.19"

FROM golang:${GO_VERSION}-alpine AS build
WORKDIR /src
ENV CGO_ENABLED=0 \
    GOOS=linux \
    GOFLAGS="-trimpath -mod=readonly"
COPY go.mod go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod \
    go mod download
COPY . .
RUN go build -ldflags "-s -w -X main.version=${VERSION:-dev}" -o /out/server ./cmd/server && \
    echo 'built' # trailing comment
RUN echo "unterminated double
RUN echo 'unterminated single
RUN echo "escaped \" quote" 'and \' single' plain `backtick`
RUN echo "string in the middle" and more text
RUN echo 'single in the middle' and more

FROM ${BASE_IMAGE}
from scratch as final
From alpine
FROM   # an error comment
FROM
LABEL org.opencontainers.image.title="server" \
      org.opencontainers.image.description="A tiny job queue"
MAINTAINER someone@example.com
EXPOSE 8080
EXPOSE 8080/tcp 9090/udp
expose 7000 7001
EXPOSE $PORT
USER nobody:nobody
VOLUME ["/data", "/logs"]
COPY --from=build /out/server /usr/local/bin/server
ADD https://example.com/file.tar.gz /tmp/
HEALTHCHECK --interval=30s --timeout=3s \
  CMD wget -qO- http://localhost:8080/health || exit 1
STOPSIGNAL SIGTERM
SHELL ["/bin/sh", "-c"]
ONBUILD RUN echo "onbuild"
ENTRYPOINT ["/usr/local/bin/server", "--port", "8080"]
CMD ["--verbose", "--config=\"/etc/app.conf\""]
cmd [ "unterminated
RUN
ENV # an instruction with only a comment
	RUN indented with a tab
  # an indented comment
not an instruction line
RUN echo "multi \
  line" \
  && echo done
