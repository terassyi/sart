#!/bin/bash
set -e

cd sartd/test
go mod tidy
go test -v
