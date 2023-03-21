#!/bin/bash

grpcurl -plaintext -d '{"peer":{"asn": 200,"address":"10.0.0.2","router_id":"2.2.2.2","families":[{"afi":1,"safi":1}],"hold_time":180,"keepalive_time":60}}' localhost:5000 sart.v1.BgpApi.AddPeer
grpcurl -plaintext -d '{"peer":{"asn": 300,"address":"10.0.1.2","router_id":"3.3.3.3","families":[{"afi":1,"safi":1}],"hold_time":180,"keepalive_time":60}}' localhost:5000 sart.v1.BgpApi.AddPeer
