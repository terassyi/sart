#!/bin/bash

grpcurl -plaintext -d '{"prefixes": ["10.0.0.0/24"]}' localhost:5000 sart.v1.BgpApi.AddPath
grpcurl -plaintext -d '{"prefixes": ["10.1.0.0/24", "10.2.0.0/24"], "attributes": [{"@type":"type.googleapis.com/sart.v1.OriginAttribute", "value": 2}]}' localhost:5000 sart.v1.BgpApi.AddPath

