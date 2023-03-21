#!/bin/bash

grpcurl -plaintext -d '{"asn": 100}' localhost:5000 sart.v1.BgpApi.SetAS
grpcurl -plaintext -d '{"routerId": "1.1.1.1"}' localhost:5000 sart.v1.BgpApi.SetRouterId
