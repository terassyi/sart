package allocator

import (
	"fmt"
	"math/big"
	"net/netip"

	"github.com/bits-and-blooms/bitset"
)

type Allocator interface {
	Allocate(netip.Addr) (bool, error)
}

func New(cidr *netip.Prefix) Allocator {
	return newAllocator(cidr)
}

type allocator struct {
	cidr *netip.Prefix
	bits *bitset.BitSet
}

func newAllocator(cidr *netip.Prefix) *allocator {
	b := func() uint {
		if cidr.Addr().Is4() {
			return 32
		} else {
			return 128
		}
	}()
	return &allocator{
		cidr: cidr,
		bits: bitset.New(uint(1)<<b - uint(cidr.Bits())),
	}
}

func (a *allocator) Allocate(addr netip.Addr) (bool, error) {
	if !a.cidr.Contains(addr) {
		return false, fmt.Errorf("requested address isn't contained CIDR range")
	}
	return true, nil
}

func diffAddr(a, b netip.Addr) int64 {
	aa := new(big.Int)
	bb := new(big.Int)
	aa.SetBytes(a.AsSlice())
	bb.SetBytes(b.AsSlice())
	return aa.Sub(aa, bb).Int64()
}
