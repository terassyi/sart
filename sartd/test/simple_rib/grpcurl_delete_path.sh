#!/bin/bash

grpcurl -plaintext -d '{"prefixes": ["10.0.0.0/24"]}' localhost:5000 sart.v1.BgpApi.DeletePath
