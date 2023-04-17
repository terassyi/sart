FROM golang:1.20

WORKDIR /work
COPY main.go /work/main.go
COPY go.mod /work/go.mod

RUN go build -a -o app main.go

CMD [ "/work/app", "-port", "80" ]
