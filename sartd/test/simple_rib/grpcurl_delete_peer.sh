#!/bin/bash

grpcurl -plaintext -d '{"addr": "10.0.1.2"}' localhost:5000 sart.BgpApi.DeletePeer
