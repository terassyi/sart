# Design of sart

Sart は自作のルーティングソフトウェア

## Concept

## sartd-bgp

BGP の実装は [RFC 4271](https://www.rfc-editor.org/rfc/rfc4271)に基づく．
gRPC インターフェースを備えて外部より設定を注入できる．

#### 機能

目指す機能．
- [ ] eBGP
- [ ] iBGP
- [ ] 4 byte ASN
- [ ] IPv4 and IPv6 dual stack
	- [ ] Multiprotocol Extentions for BGP-4 [RFC 4760](https://www.rfc-editor.org/rfc/rfc4760)
	- [ ] BGP4+(IPv6 拡張) に関する [RFC 2546](https://datatracker.ietf.org/doc/html/rfc2546)
- [ ] Route Reflector
- [ ] BGP unnumbered

