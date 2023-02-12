#!/bin/bash

grpcurl -plaintext -d '{"asn": 100}' localhost:5000 sart.BgpApi.SetAS
grpcurl -plaintext -d '{"routerId": "1.1.1.1"}' localhost:5000 sart.BgpApi.SetRouterId
