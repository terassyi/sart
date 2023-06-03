FROM --platform=$BUILDPLATFORM golang:1.20 as builder

ARG TARGETOS
ARG TARGETARCH

WORKDIR /work
COPY main.go /work/main.go
COPY go.mod /work/go.mod

RUN CGO_ENABLED=0 GOOS=${TARGETOS:-linux} GOARCH=${TARGETARCH} go build -a -o app main.go

FROM --platform=$TARGETPLATFORM scratch

COPY --from=builder /work/app /app

ENTRYPOINT [ "/app", "-port", "80" ]
